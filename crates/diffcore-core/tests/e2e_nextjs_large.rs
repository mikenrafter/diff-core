#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
//! E2E integration tests for Next.js, large diffs, staged changes, and config overrides.

mod helpers;

use diffcore_core::git;
use diffcore_core::output;
use helpers::graph_assertions::{
    assert_all_files_accounted, assert_json_roundtrip, assert_language_detected,
    assert_valid_json_schema, assert_valid_scores,
};
use helpers::repo_builder::{run_pipeline, RepoBuilder};

// ─── Spec §13.5 Missing Integration Tests ─────────────────────────────

/// Test: Next.js app — modify a page + API route + Prisma model.
///
/// Creates a Next.js fullstack app with API routes and React pages.
/// Expected: produces 2 groups (API flow and UI flow), correctly separated.
#[test]
fn test_e2e_nextjs_page_change() {
    let rb = RepoBuilder::new();

    // Initial commit: base Next.js app with existing page + API route
    rb.write_file(
        "package.json",
        r#"{"name": "nextjs-app", "dependencies": {"next": "14.0.0", "@prisma/client": "5.0.0"}}"#,
    );
    rb.write_file(
        "src/app/api/users/route.ts",
        r#"
import { NextResponse } from 'next/server';

export async function GET() {
    return NextResponse.json([]);
}
"#,
    );
    rb.write_file(
        "src/app/users/page.tsx",
        r#"
export default function UsersPage() {
    return <div>Users</div>;
}
"#,
    );
    rb.write_file(
        "prisma/schema.prisma",
        r#"
model User {
  id    String @id
  name  String
}
"#,
    );
    rb.commit("Initial Next.js app");
    rb.create_branch("main");

    rb.create_branch("feature/nextjs-products");
    rb.checkout("feature/nextjs-products");

    // Add new API route (products)
    rb.write_file(
        "src/app/api/products/route.ts",
        r#"
import { NextResponse } from 'next/server';
import { getProducts, createProduct } from '../../../services/productService';

export async function GET() {
    const products = await getProducts();
    return NextResponse.json(products);
}

export async function POST(request: Request) {
    const body = await request.json();
    const product = await createProduct(body);
    return NextResponse.json(product, { status: 201 });
}
"#,
    );

    // Add service layer
    rb.write_file(
        "src/services/productService.ts",
        r#"
import { PrismaClient } from '@prisma/client';

const prisma = new PrismaClient();

export async function getProducts() {
    return prisma.product.findMany();
}

export async function createProduct(data: any) {
    return prisma.product.create({ data });
}
"#,
    );

    // Add new page (products listing)
    rb.write_file(
        "src/app/products/page.tsx",
        r#"
import { ProductList } from '../../components/ProductList';

export default async function ProductsPage() {
    const res = await fetch('/api/products');
    const products = await res.json();
    return <ProductList products={products} />;
}
"#,
    );

    // Add component
    rb.write_file(
        "src/components/ProductList.tsx",
        r#"
export function ProductList({ products }: { products: any[] }) {
    return (
        <ul>
            {products.map(p => <li key={p.id}>{p.name}</li>)}
        </ul>
    );
}
"#,
    );

    // Update Prisma schema (add Product model)
    rb.write_file(
        "prisma/schema.prisma",
        r#"
model User {
  id    String @id
  name  String
}

model Product {
  id    String @id
  name  String
  price Float
}
"#,
    );

    rb.commit("Add product listing: API route + page + Prisma model");

    let output = run_pipeline(rb.path(), "main", "feature/nextjs-products");

    // Should have at least 4 changed files (API route, service, page, component, schema)
    assert!(
        output.summary.total_files_changed >= 4,
        "expected >= 4 changed files, got {}",
        output.summary.total_files_changed
    );

    // Should produce at least 2 groups (API flow and UI flow)
    assert!(
        output.summary.total_groups >= 2,
        "expected >= 2 flow groups for API + UI flows, got {}",
        output.summary.total_groups
    );

    assert_language_detected(&output, "typescript");
    assert_all_files_accounted(&output);
    assert_json_roundtrip(&output);
}

/// Test: 50-file diff completes within reasonable time and produces valid output.
///
/// Generates 50 TypeScript files with import chains and verifies
/// the pipeline handles them without timeout or OOM.
#[test]
fn test_e2e_50_file_diff() {
    let rb = RepoBuilder::new();

    rb.write_file("package.json", r#"{"name": "large-app-50"}"#);
    rb.commit("Initial");
    rb.create_branch("main");

    rb.create_branch("feature/50-files");
    rb.checkout("feature/50-files");

    // Create 50 files: 5 route entrypoints, each with a chain of 9 downstream files
    for group in 0..5 {
        let route_file = format!("src/routes/route{}.ts", group);
        rb.write_file(
            &route_file,
            &format!(
                "import express from 'express';\nimport {{ service{g} }} from '../services/svc{g}';\n\nconst router = express.Router();\nexport function handle{g}(req: any, res: any) {{ res.json(service{g}()); }}\nrouter.get('/route{g}', handle{g});\nexport default router;\n",
                g = group
            ),
        );
        for depth in 0..9 {
            let file = format!("src/services/svc{}_{}.ts", group, depth);
            let content = if depth == 0 {
                format!(
                    "import {{ fn{}_{} }} from './svc{}_{}';\nexport function service{}() {{ return fn{}_{}(); }}\n",
                    group, depth + 1, group, depth + 1, group, group, depth + 1
                )
            } else if depth < 8 {
                format!(
                    "import {{ fn{}_{} }} from './svc{}_{}';\nexport function fn{}_{}() {{ return fn{}_{}(); }}\n",
                    group, depth + 1, group, depth + 1, group, depth, group, depth + 1
                )
            } else {
                format!(
                    "export function fn{}_{}() {{ return {{ value: {} }}; }}\n",
                    group,
                    depth,
                    group * 10 + depth
                )
            };
            rb.write_file(&file, &content);
        }
    }
    rb.commit("Add 50 files across 5 route groups");

    let start = std::time::Instant::now();
    let output = run_pipeline(rb.path(), "main", "feature/50-files");
    let elapsed = start.elapsed();

    assert_eq!(
        output.summary.total_files_changed, 50,
        "expected 50 changed files"
    );
    assert!(
        elapsed.as_secs() < 5,
        "50-file analysis should complete in <5s, took {:?}",
        elapsed
    );
    assert_all_files_accounted(&output);
    assert_valid_scores(&output);

    let json = output::to_json(&output).unwrap();
    let _: serde_json::Value = serde_json::from_str(&json).unwrap();
}

/// Test: 100-file diff completes within reasonable time without OOM.
///
/// Generates 100 TypeScript files with import chains. Verifies the
/// pipeline scales to large diffs without panicking or timing out.
#[test]
fn test_e2e_100_file_diff() {
    let rb = RepoBuilder::new();

    rb.write_file("package.json", r#"{"name": "large-app-100"}"#);
    rb.commit("Initial");
    rb.create_branch("main");

    rb.create_branch("feature/100-files");
    rb.checkout("feature/100-files");

    // Create 100 files: 10 route entrypoints, each with a chain of 9 downstream files
    for group in 0..10 {
        let route_file = format!("src/routes/route{}.ts", group);
        rb.write_file(
            &route_file,
            &format!(
                "import express from 'express';\nimport {{ svc{g} }} from '../services/svc{g}';\n\nconst router = express.Router();\nexport function handler{g}(req: any, res: any) {{ res.json(svc{g}()); }}\nrouter.get('/r{g}', handler{g});\nexport default router;\n",
                g = group
            ),
        );
        for depth in 0..9 {
            let file = format!("src/services/svc{}_{}.ts", group, depth);
            let content = if depth == 0 {
                format!(
                    "import {{ f{}_{} }} from './svc{}_{}';\nexport function svc{}() {{ return f{}_{}(); }}\n",
                    group, depth + 1, group, depth + 1, group, group, depth + 1
                )
            } else if depth < 8 {
                format!(
                    "import {{ f{}_{} }} from './svc{}_{}';\nexport function f{}_{}() {{ return f{}_{}(); }}\n",
                    group, depth + 1, group, depth + 1, group, depth, group, depth + 1
                )
            } else {
                format!(
                    "export function f{}_{}() {{ return {{ v: {} }}; }}\n",
                    group,
                    depth,
                    group * 10 + depth
                )
            };
            rb.write_file(&file, &content);
        }
    }
    rb.commit("Add 100 files across 10 route groups");

    let start = std::time::Instant::now();
    let output = run_pipeline(rb.path(), "main", "feature/100-files");
    let elapsed = start.elapsed();

    assert_eq!(
        output.summary.total_files_changed, 100,
        "expected 100 changed files"
    );
    assert!(
        elapsed.as_secs() < 15,
        "100-file analysis should complete in <15s, took {:?}",
        elapsed
    );
    assert_all_files_accounted(&output);
    assert_valid_scores(&output);

    // Should produce valid JSON
    let json = output::to_json(&output).unwrap();
    let _: serde_json::Value = serde_json::from_str(&json).unwrap();
}

/// Test: Staged changes — only staged files are included in the diff.
///
/// Stages some files but leaves others unstaged, then runs diff_staged
/// and verifies only the staged files appear in the result.
#[test]
fn test_e2e_staged_changes() {
    let rb = RepoBuilder::new();

    // Create initial files
    rb.write_file(
        "src/handler.ts",
        "export function handle() { return 'v1'; }\n",
    );
    rb.write_file(
        "src/service.ts",
        "export function serve() { return 'v1'; }\n",
    );
    rb.write_file("src/utils.ts", "export function util() { return 'v1'; }\n");
    rb.commit("Initial commit");

    // Modify all three files in the working directory
    rb.write_file(
        "src/handler.ts",
        "export function handle() { return 'v2-staged'; }\n",
    );
    rb.write_file(
        "src/service.ts",
        "export function serve() { return 'v2-staged'; }\n",
    );
    rb.write_file(
        "src/utils.ts",
        "export function util() { return 'v2-unstaged'; }\n",
    );

    // Stage only handler.ts and service.ts (NOT utils.ts)
    {
        let repo = rb.repo();
        let mut index = repo.index().unwrap();
        index
            .add_path(std::path::Path::new("src/handler.ts"))
            .unwrap();
        index
            .add_path(std::path::Path::new("src/service.ts"))
            .unwrap();
        index.write().unwrap();
    }

    // Run diff_staged
    let repo = git2::Repository::open(rb.path()).unwrap();
    let diff_result = git::diff_staged(&repo).unwrap();

    // Should include exactly the 2 staged files
    let staged_paths: Vec<&str> = diff_result.files.iter().map(|f| f.path()).collect();
    assert_eq!(
        staged_paths.len(),
        2,
        "expected 2 staged files, got {:?}",
        staged_paths
    );
    assert!(
        staged_paths.contains(&"src/handler.ts"),
        "handler.ts should be staged"
    );
    assert!(
        staged_paths.contains(&"src/service.ts"),
        "service.ts should be staged"
    );
    assert!(
        !staged_paths.contains(&"src/utils.ts"),
        "utils.ts should NOT be in staged diff"
    );

    // Verify the staged content is correct
    for file_diff in &diff_result.files {
        if let Some(ref new_content) = file_diff.new_content {
            assert!(
                new_content.contains("v2-staged"),
                "staged file {} should have v2-staged content",
                file_diff.path()
            );
        }
    }
}

/// Test: Config overrides — custom entrypoint globs detect files that heuristics miss.
///
/// Creates files in non-standard locations that aren't detected by built-in
/// heuristics, then provides a `.diffcore.toml` with custom entrypoint globs
/// that should pick them up.
#[test]
fn test_e2e_config_overrides() {
    let rb = RepoBuilder::new();

    rb.write_file("package.json", r#"{"name": "custom-app"}"#);
    rb.commit("Initial");
    rb.create_branch("main");

    rb.create_branch("feature/custom-ep");
    rb.checkout("feature/custom-ep");

    // Files in non-standard locations (no framework imports, no standard paths)
    // These would NOT be detected as entrypoints by default heuristics
    rb.write_file(
        "src/triggers/onUserCreated.ts",
        r#"
import { notifyAdmin } from '../notifications/admin';

export function handleUserCreated(event: any) {
    notifyAdmin(event.userId);
    return { processed: true };
}
"#,
    );
    rb.write_file(
        "src/triggers/onOrderPlaced.ts",
        r#"
import { processPayment } from '../billing/payment';

export function handleOrderPlaced(event: any) {
    processPayment(event.orderId, event.amount);
    return { processed: true };
}
"#,
    );
    rb.write_file(
        "src/notifications/admin.ts",
        r#"
export function notifyAdmin(userId: string) {
    console.log('Notifying admin about user:', userId);
}
"#,
    );
    rb.write_file(
        "src/billing/payment.ts",
        r#"
export function processPayment(orderId: string, amount: number) {
    console.log('Processing payment:', orderId, amount);
}
"#,
    );

    // Provide a config that declares triggers as event entrypoints
    rb.write_file(
        ".diffcore.toml",
        r#"
[entrypoints]
events = ["src/triggers/**/*.ts"]
"#,
    );

    rb.commit("Add triggers with custom config");

    // Run pipeline WITH config loaded from repo
    let config = diffcore_core::config::DiffcoreConfig::load_from_dir(rb.path()).unwrap();

    // Verify config was loaded and has our custom entrypoints
    assert!(
        !config.entrypoints.events.is_empty(),
        "config should have event entrypoint globs"
    );

    // Resolve entrypoint globs
    let resolved = config.resolve_entrypoint_globs(rb.path());
    assert!(
        resolved
            .iter()
            .any(|p| p.to_string_lossy().contains("triggers")),
        "config should resolve trigger files; resolved: {:?}",
        resolved
    );

    // Run full pipeline and verify files are accounted for
    let output = run_pipeline(rb.path(), "main", "feature/custom-ep");

    assert!(
        output.summary.total_files_changed >= 4,
        "expected >= 4 changed files, got {}",
        output.summary.total_files_changed
    );
    assert_all_files_accounted(&output);
    assert_valid_json_schema(&output);
}

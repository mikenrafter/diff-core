    use super::*;
    use crate::ast::{CallSite, Definition, ExportInfo, ImportInfo, ImportedName};
    use crate::types::SymbolKind;

    fn make_file(path: &str, lang: Language) -> ParsedFile {
        ParsedFile {
            path: path.to_string(),
            language: lang,
            definitions: vec![],
            imports: vec![],
            exports: vec![],
            call_sites: vec![],
        }
    }

    fn make_def(name: &str, kind: SymbolKind) -> Definition {
        Definition {
            name: name.to_string(),
            kind,
            start_line: 1,
            end_line: 5,
        }
    }

    fn make_import(source: &str) -> ImportInfo {
        ImportInfo {
            source: source.to_string(),
            names: vec![],
            is_default: false,
            is_namespace: false,
            line: 1,
        }
    }

    fn make_import_with_names(source: &str, names: Vec<&str>) -> ImportInfo {
        ImportInfo {
            source: source.to_string(),
            names: names
                .into_iter()
                .map(|n| ImportedName {
                    name: n.to_string(),
                    alias: None,
                })
                .collect(),
            is_default: false,
            is_namespace: false,
            line: 1,
        }
    }

    fn make_export(name: &str, is_default: bool) -> ExportInfo {
        ExportInfo {
            name: name.to_string(),
            is_default,
            is_reexport: false,
            source: None,
            line: 1,
        }
    }

    fn make_call(callee: &str, containing: Option<&str>) -> CallSite {
        CallSite {
            callee: callee.to_string(),
            line: 1,
            containing_function: containing.map(|s| s.to_string()),
        }
    }

    // ========================================================================
    // Test file detection
    // ========================================================================

    #[test]
    fn test_detect_test_file_by_path_dot_test() {
        let file = make_file("src/utils.test.ts", Language::TypeScript);
        let result = detect_entrypoints(&[file]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].entrypoint_type, EntrypointType::TestFile);
        assert_eq!(result[0].symbol, "utils");
    }

    #[test]
    fn test_detect_test_file_by_path_dot_spec() {
        let file = make_file("src/utils.spec.js", Language::JavaScript);
        let result = detect_entrypoints(&[file]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].entrypoint_type, EntrypointType::TestFile);
    }

    #[test]
    fn test_detect_test_file_python_prefix() {
        let file = make_file("tests/test_utils.py", Language::Python);
        let result = detect_entrypoints(&[file]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].entrypoint_type, EntrypointType::TestFile);
    }

    #[test]
    fn test_detect_test_file_tests_directory() {
        let file = make_file("__tests__/App.test.tsx", Language::TypeScript);
        let result = detect_entrypoints(&[file]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].entrypoint_type, EntrypointType::TestFile);
    }

    #[test]
    fn test_detect_test_file_with_test_functions() {
        let mut file = make_file("tests/test_auth.py", Language::Python);
        file.definitions = vec![
            make_def("test_login", SymbolKind::Function),
            make_def("test_logout", SymbolKind::Function),
            make_def("helper_setup", SymbolKind::Function),
        ];
        let result = detect_entrypoints(&[file]);
        // Should detect test_login and test_logout but not helper_setup
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|e| e.symbol == "test_login"));
        assert!(result.iter().any(|e| e.symbol == "test_logout"));
    }

    #[test]
    fn test_non_test_file_not_detected() {
        let file = make_file("src/utils.ts", Language::TypeScript);
        let result = detect_entrypoints(&[file]);
        assert!(result.is_empty());
    }

    // ========================================================================
    // HTTP route detection — JS/TS
    // ========================================================================

    #[test]
    fn test_detect_express_route() {
        let mut file = make_file("src/routes/users.ts", Language::TypeScript);
        file.call_sites = vec![
            make_call("app.get", Some("setupRoutes")),
            make_call("app.post", Some("setupRoutes")),
        ];
        let result = detect_entrypoints(&[file]);
        assert!(!result.is_empty());
        assert!(result
            .iter()
            .all(|e| e.entrypoint_type == EntrypointType::HttpRoute));
    }

    #[test]
    fn test_detect_router_route() {
        let mut file = make_file("src/routes/api.ts", Language::TypeScript);
        file.call_sites = vec![make_call("router.get", Some("getUsers"))];
        let result = detect_entrypoints(&[file]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].symbol, "getUsers");
        assert_eq!(result[0].entrypoint_type, EntrypointType::HttpRoute);
    }

    #[test]
    fn test_detect_nextjs_app_router_route() {
        let mut file = make_file("src/app/api/users/route.ts", Language::TypeScript);
        file.exports = vec![make_export("GET", false), make_export("POST", false)];
        let result = detect_entrypoints(&[file]);
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|e| e.symbol == "GET"));
        assert!(result.iter().any(|e| e.symbol == "POST"));
        assert!(result
            .iter()
            .all(|e| e.entrypoint_type == EntrypointType::HttpRoute));
    }

    #[test]
    fn test_detect_nextjs_pages_router() {
        let mut file = make_file("pages/about.tsx", Language::TypeScript);
        file.exports = vec![make_export("AboutPage", true)];
        let result = detect_entrypoints(&[file]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].symbol, "AboutPage");
        assert_eq!(result[0].entrypoint_type, EntrypointType::HttpRoute);
    }

    #[test]
    fn test_nextjs_pages_skip_internal_files() {
        let mut file = make_file("pages/_app.tsx", Language::TypeScript);
        file.exports = vec![make_export("App", true)];
        let result = detect_entrypoints(&[file]);
        // _app.tsx should NOT be detected as a page route
        assert!(result
            .iter()
            .all(|e| e.entrypoint_type != EntrypointType::HttpRoute));
    }

    #[test]
    fn test_non_route_call_not_detected() {
        let mut file = make_file("src/utils.ts", Language::TypeScript);
        file.call_sites = vec![make_call("console.log", Some("debug"))];
        let result = detect_entrypoints(&[file]);
        assert!(result.is_empty());
    }

    // ========================================================================
    // HTTP route detection — Python
    // ========================================================================

    #[test]
    fn test_detect_flask_route() {
        let mut file = make_file("src/routes.py", Language::Python);
        file.imports = vec![make_import_with_names("flask", vec!["Flask"])];
        file.call_sites = vec![make_call("app.route", Some("list_users"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "list_users" && e.entrypoint_type == EntrypointType::HttpRoute));
    }

    #[test]
    fn test_detect_fastapi_route() {
        let mut file = make_file("src/routes.py", Language::Python);
        file.imports = vec![make_import_with_names("fastapi", vec!["FastAPI"])];
        file.call_sites = vec![make_call("app.get", Some("get_users"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "get_users" && e.entrypoint_type == EntrypointType::HttpRoute));
    }

    #[test]
    fn test_detect_python_views_module() {
        let mut file = make_file("myapp/views.py", Language::Python);
        file.imports = vec![make_import("django.http")];
        file.definitions = vec![
            make_def("index", SymbolKind::Function),
            make_def("detail", SymbolKind::Function),
            make_def("__init__", SymbolKind::Function),
            make_def("_helper", SymbolKind::Function),
        ];
        let result = detect_entrypoints(&[file]);
        // Should detect index and detail, but not __init__ or _helper
        let http_routes: Vec<_> = result
            .iter()
            .filter(|e| e.entrypoint_type == EntrypointType::HttpRoute)
            .collect();
        assert_eq!(http_routes.len(), 2);
        assert!(http_routes.iter().any(|e| e.symbol == "index"));
        assert!(http_routes.iter().any(|e| e.symbol == "detail"));
    }

    // ========================================================================
    // CLI command detection
    // ========================================================================

    #[test]
    fn test_detect_python_main() {
        let mut file = make_file("src/cli.py", Language::Python);
        file.definitions = vec![make_def("main", SymbolKind::Function)];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "main" && e.entrypoint_type == EntrypointType::CliCommand));
    }

    #[test]
    fn test_detect_ts_main_in_cli_path() {
        let mut file = make_file("src/cli/main.ts", Language::TypeScript);
        file.definitions = vec![make_def("main", SymbolKind::Function)];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "main" && e.entrypoint_type == EntrypointType::CliCommand));
    }

    #[test]
    fn test_detect_commander_cli() {
        let mut file = make_file("src/cli.ts", Language::TypeScript);
        file.imports = vec![make_import("commander")];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.entrypoint_type == EntrypointType::CliCommand));
    }

    #[test]
    fn test_detect_click_cli() {
        let mut file = make_file("src/main.py", Language::Python);
        file.imports = vec![make_import("click")];
        file.definitions = vec![make_def("main", SymbolKind::Function)];
        file.call_sites = vec![make_call("click.command", Some("main"))];
        let result = detect_entrypoints(&[file]);
        let cli_entries: Vec<_> = result
            .iter()
            .filter(|e| e.entrypoint_type == EntrypointType::CliCommand)
            .collect();
        assert!(!cli_entries.is_empty());
    }

    #[test]
    fn test_detect_bin_path_as_cli() {
        let file = make_file("bin/run.js", Language::JavaScript);
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.entrypoint_type == EntrypointType::CliCommand));
    }

    #[test]
    fn test_main_in_non_cli_path_not_cli_for_ts() {
        // A main() in a random TS file shouldn't be CLI
        let mut file = make_file("src/components/Widget.ts", Language::TypeScript);
        file.definitions = vec![make_def("main", SymbolKind::Function)];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .all(|e| e.entrypoint_type != EntrypointType::CliCommand));
    }

    // ========================================================================
    // Queue consumer detection
    // ========================================================================

    #[test]
    fn test_detect_bull_queue_consumer() {
        let mut file = make_file("src/workers/email.ts", Language::TypeScript);
        file.imports = vec![make_import("bullmq")];
        file.call_sites = vec![make_call("queue.process", Some("processEmail"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.entrypoint_type == EntrypointType::QueueConsumer));
    }

    #[test]
    fn test_detect_celery_consumer() {
        let mut file = make_file("src/tasks/send_email.py", Language::Python);
        file.imports = vec![make_import("celery")];
        file.definitions = vec![make_def("process_email", SymbolKind::Function)];
        // Worker path + celery import → queue consumer for process-like functions
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.entrypoint_type == EntrypointType::QueueConsumer));
    }

    #[test]
    fn test_no_queue_without_import() {
        let mut file = make_file("src/workers/email.ts", Language::TypeScript);
        file.call_sites = vec![make_call("queue.process", Some("processEmail"))];
        // No queue import → no detection
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .all(|e| e.entrypoint_type != EntrypointType::QueueConsumer));
    }

    // ========================================================================
    // Cron job detection
    // ========================================================================

    #[test]
    fn test_detect_node_cron() {
        let mut file = make_file("src/cron/cleanup.ts", Language::TypeScript);
        file.imports = vec![make_import("node-cron")];
        file.call_sites = vec![make_call("cron.schedule", Some("scheduleCleanup"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.entrypoint_type == EntrypointType::CronJob));
    }

    #[test]
    fn test_detect_apscheduler() {
        let mut file = make_file("src/scheduler/jobs.py", Language::Python);
        file.imports = vec![make_import("apscheduler")];
        file.call_sites = vec![make_call("scheduler.add_job", Some("daily_report"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.entrypoint_type == EntrypointType::CronJob));
    }

    // ========================================================================
    // React page detection
    // ========================================================================

    #[test]
    fn test_detect_nextjs_page_tsx() {
        let mut file = make_file("src/app/dashboard/page.tsx", Language::TypeScript);
        file.exports = vec![make_export("DashboardPage", true)];
        let result = detect_entrypoints(&[file]);
        assert!(
            result
                .iter()
                .any(|e| e.symbol == "DashboardPage"
                    && e.entrypoint_type == EntrypointType::ReactPage)
        );
    }

    #[test]
    fn test_detect_pages_dir_page() {
        let mut file = make_file("pages/dashboard.tsx", Language::TypeScript);
        file.exports = vec![make_export("Dashboard", true)];
        let result = detect_entrypoints(&[file]);
        // Should be detected as either HttpRoute (from pages router detection) or ReactPage
        assert!(!result.is_empty());
    }

    #[test]
    fn test_python_file_not_react_page() {
        let mut file = make_file("pages/admin.py", Language::Python);
        file.exports = vec![];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .all(|e| e.entrypoint_type != EntrypointType::ReactPage));
    }

    // ========================================================================
    // Event handler detection
    // ========================================================================

    #[test]
    fn test_detect_socket_event_handler() {
        let mut file = make_file("src/socket/handler.ts", Language::TypeScript);
        file.imports = vec![make_import("socket.io")];
        file.call_sites = vec![make_call("socket.on", Some("handleConnection"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.entrypoint_type == EntrypointType::EventHandler));
    }

    #[test]
    fn test_detect_eventemitter_handler() {
        let mut file = make_file("src/events/listener.ts", Language::TypeScript);
        file.imports = vec![make_import("events")];
        file.call_sites = vec![make_call("emitter.addListener", Some("onUserCreated"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.entrypoint_type == EntrypointType::EventHandler));
    }

    #[test]
    fn test_no_event_handler_without_import() {
        let mut file = make_file("src/events/listener.ts", Language::TypeScript);
        file.call_sites = vec![make_call("emitter.on", Some("handler"))];
        // No event import → no detection
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .all(|e| e.entrypoint_type != EntrypointType::EventHandler));
    }

    // ========================================================================
    // Multi-entrypoint and deduplication
    // ========================================================================

    #[test]
    fn test_multiple_files_multiple_entrypoints() {
        let mut route_file = make_file("src/routes/users.ts", Language::TypeScript);
        route_file.call_sites = vec![
            make_call("router.get", Some("getUsers")),
            make_call("router.post", Some("createUser")),
        ];

        let test_file = make_file("src/routes/users.test.ts", Language::TypeScript);

        let mut cli_file = make_file("src/cli/main.ts", Language::TypeScript);
        cli_file.definitions = vec![make_def("main", SymbolKind::Function)];

        let result = detect_entrypoints(&[route_file, test_file, cli_file]);

        let types: Vec<_> = result.iter().map(|e| &e.entrypoint_type).collect();
        assert!(types.contains(&&EntrypointType::HttpRoute));
        assert!(types.contains(&&EntrypointType::TestFile));
        assert!(types.contains(&&EntrypointType::CliCommand));
    }

    #[test]
    fn test_deduplication() {
        // A file that could trigger the same entrypoint via multiple detection paths
        let mut file = make_file("src/app/api/users/route.ts", Language::TypeScript);
        file.exports = vec![make_export("GET", false)];

        let result = detect_entrypoints(&[file]);
        // Should not have duplicates
        let get_entries: Vec<_> = result
            .iter()
            .filter(|e| e.symbol == "GET" && e.file == "src/app/api/users/route.ts")
            .collect();
        assert_eq!(get_entries.len(), 1);
    }

    #[test]
    fn test_empty_input() {
        let result = detect_entrypoints(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_no_entrypoints_in_plain_utility() {
        let mut file = make_file("src/utils/format.ts", Language::TypeScript);
        file.definitions = vec![
            make_def("formatDate", SymbolKind::Function),
            make_def("formatCurrency", SymbolKind::Function),
        ];
        file.imports = vec![make_import("date-fns")];
        let result = detect_entrypoints(&[file]);
        assert!(result.is_empty());
    }

    // ========================================================================
    // Edge cases
    // ========================================================================

    // ========================================================================
    // Effect.ts HTTP route detection
    // ========================================================================

    #[test]
    fn test_detect_effect_http_api_endpoint() {
        let mut file = make_file("src/api/users.ts", Language::TypeScript);
        file.imports = vec![make_import_with_names(
            "@effect/platform",
            vec!["HttpApiEndpoint", "HttpApi"],
        )];
        file.call_sites = vec![
            make_call("HttpApiEndpoint.get", Some("getUserEndpoint")),
            make_call("HttpApiEndpoint.post", Some("createUserEndpoint")),
        ];
        let result = detect_entrypoints(&[file]);
        let http: Vec<_> = result
            .iter()
            .filter(|e| e.entrypoint_type == EntrypointType::HttpRoute)
            .collect();
        assert_eq!(http.len(), 2);
        assert!(http.iter().any(|e| e.symbol == "getUserEndpoint"));
        assert!(http.iter().any(|e| e.symbol == "createUserEndpoint"));
    }

    #[test]
    fn test_detect_effect_http_api_make() {
        let mut file = make_file("src/api/index.ts", Language::TypeScript);
        file.imports = vec![make_import("@effect/platform/HttpApi")];
        file.call_sites = vec![make_call("HttpApi.make", Some("makeApi"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "makeApi" && e.entrypoint_type == EntrypointType::HttpRoute));
    }

    #[test]
    fn test_detect_effect_http_api_group() {
        let mut file = make_file("src/api/group.ts", Language::TypeScript);
        file.imports = vec![make_import("@effect/platform/HttpApiGroup")];
        file.call_sites = vec![make_call("HttpApiGroup.make", Some("usersGroup"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "usersGroup" && e.entrypoint_type == EntrypointType::HttpRoute));
    }

    #[test]
    fn test_detect_effect_http_router() {
        let mut file = make_file("src/router.ts", Language::TypeScript);
        file.imports = vec![make_import_with_names(
            "@effect/platform",
            vec!["HttpRouter"],
        )];
        file.call_sites = vec![
            make_call("HttpRouter.get", Some("getHandler")),
            make_call("HttpRouter.post", Some("postHandler")),
        ];
        let result = detect_entrypoints(&[file]);
        let http: Vec<_> = result
            .iter()
            .filter(|e| e.entrypoint_type == EntrypointType::HttpRoute)
            .collect();
        assert_eq!(http.len(), 2);
        assert!(http.iter().any(|e| e.symbol == "getHandler"));
        assert!(http.iter().any(|e| e.symbol == "postHandler"));
    }

    #[test]
    fn test_detect_effect_http_subpath_import() {
        let mut file = make_file("src/api/endpoint.ts", Language::TypeScript);
        file.imports = vec![make_import("@effect/platform/HttpApiEndpoint")];
        file.call_sites = vec![make_call("HttpApiEndpoint.put", Some("updateUser"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "updateUser" && e.entrypoint_type == EntrypointType::HttpRoute));
    }

    #[test]
    fn test_no_effect_http_without_import() {
        let mut file = make_file("src/api/users.ts", Language::TypeScript);
        file.call_sites = vec![make_call("HttpApiEndpoint.get", Some("getUser"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .all(|e| e.entrypoint_type != EntrypointType::HttpRoute));
    }

    // ========================================================================
    // Effect.ts CLI command detection
    // ========================================================================

    #[test]
    fn test_detect_effect_cli_command_make() {
        let mut file = make_file("src/cli/main.ts", Language::TypeScript);
        file.imports = vec![make_import_with_names(
            "@effect/cli",
            vec!["Command", "Args"],
        )];
        file.call_sites = vec![make_call("Command.make", Some("myCommand"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "myCommand" && e.entrypoint_type == EntrypointType::CliCommand));
    }

    #[test]
    fn test_detect_effect_cli_command_run() {
        let mut file = make_file("src/cli.ts", Language::TypeScript);
        file.imports = vec![make_import("@effect/cli/Command")];
        file.call_sites = vec![make_call("Command.run", Some("runCli"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "runCli" && e.entrypoint_type == EntrypointType::CliCommand));
    }

    #[test]
    fn test_no_effect_cli_without_import() {
        let mut file = make_file("src/cli.ts", Language::TypeScript);
        file.call_sites = vec![make_call("Command.make", Some("myCmd"))];
        let result = detect_entrypoints(&[file]);
        // Without @effect/cli import, should not detect via Effect.ts CLI path
        // (may detect via other paths if path matches cli patterns)
        assert!(result
            .iter()
            .all(|e| e.symbol != "myCmd" || e.entrypoint_type != EntrypointType::CliCommand));
    }

    // ========================================================================
    // Effect.ts queue consumer detection
    // ========================================================================

    #[test]
    fn test_detect_effect_queue_take() {
        let mut file = make_file("src/workers/processor.ts", Language::TypeScript);
        file.imports = vec![make_import_with_names("effect", vec!["Queue"])];
        file.call_sites = vec![make_call("Queue.take", Some("processMessages"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "processMessages"
                && e.entrypoint_type == EntrypointType::QueueConsumer));
    }

    #[test]
    fn test_detect_effect_pubsub_subscribe() {
        let mut file = make_file("src/events/subscriber.ts", Language::TypeScript);
        file.imports = vec![make_import_with_names("effect", vec!["PubSub"])];
        file.call_sites = vec![make_call("PubSub.subscribe", Some("handleEvents"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "handleEvents"
                && e.entrypoint_type == EntrypointType::QueueConsumer));
    }

    #[test]
    fn test_detect_effect_queue_subpath_import() {
        let mut file = make_file("src/worker.ts", Language::TypeScript);
        file.imports = vec![make_import("effect/Queue")];
        file.call_sites = vec![make_call("Queue.dequeue", Some("drain"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "drain" && e.entrypoint_type == EntrypointType::QueueConsumer));
    }

    // ========================================================================
    // Effect.ts cron job detection
    // ========================================================================

    #[test]
    fn test_detect_effect_schedule_cron() {
        let mut file = make_file("src/cron/cleanup.ts", Language::TypeScript);
        file.imports = vec![make_import_with_names("effect", vec!["Schedule"])];
        file.call_sites = vec![make_call("Schedule.cron", Some("dailyCleanup"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "dailyCleanup" && e.entrypoint_type == EntrypointType::CronJob));
    }

    #[test]
    fn test_detect_effect_schedule_spaced() {
        let mut file = make_file("src/scheduler.ts", Language::TypeScript);
        file.imports = vec![make_import("effect/Schedule")];
        file.call_sites = vec![make_call("Schedule.spaced", Some("heartbeat"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "heartbeat" && e.entrypoint_type == EntrypointType::CronJob));
    }

    #[test]
    fn test_detect_effect_cron_make() {
        let mut file = make_file("src/cron.ts", Language::TypeScript);
        file.imports = vec![make_import("@effect/cron")];
        file.call_sites = vec![make_call("Cron.make", Some("setupCron"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "setupCron" && e.entrypoint_type == EntrypointType::CronJob));
    }

    #[test]
    fn test_no_effect_cron_without_import() {
        let mut file = make_file("src/utils.ts", Language::TypeScript);
        file.call_sites = vec![make_call("Schedule.cron", Some("nope"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .all(|e| e.entrypoint_type != EntrypointType::CronJob));
    }

    // ========================================================================
    // Effect.ts test file detection
    // ========================================================================

    #[test]
    fn test_detect_effect_vitest_it_effect() {
        let mut file = make_file("src/services/auth.test.ts", Language::TypeScript);
        file.imports = vec![make_import("@effect/vitest")];
        file.call_sites = vec![make_call("it.effect", Some("describe"))];
        let result = detect_entrypoints(&[file]);
        // Should be detected via both test path and Effect.ts vitest
        assert!(result
            .iter()
            .any(|e| e.entrypoint_type == EntrypointType::TestFile));
    }

    #[test]
    fn test_detect_effect_vitest_it_scoped() {
        let mut file = make_file("src/services/db.test.ts", Language::TypeScript);
        file.imports = vec![make_import("@effect/vitest")];
        file.call_sites = vec![make_call("it.scoped", Some("dbTests"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.entrypoint_type == EntrypointType::TestFile));
    }

    #[test]
    fn test_detect_effect_vitest_it_live() {
        let mut file = make_file("test/integration.test.ts", Language::TypeScript);
        file.imports = vec![make_import("@effect/vitest")];
        file.call_sites = vec![make_call("it.live", Some("liveTest"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.entrypoint_type == EntrypointType::TestFile));
    }

    // ========================================================================
    // Effect.ts event handler detection
    // ========================================================================

    #[test]
    fn test_detect_effect_stream_run() {
        let mut file = make_file("src/streams/processor.ts", Language::TypeScript);
        file.imports = vec![make_import_with_names("effect", vec!["Stream"])];
        file.call_sites = vec![make_call("Stream.runForEach", Some("processStream"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "processStream"
                && e.entrypoint_type == EntrypointType::EventHandler));
    }

    #[test]
    fn test_detect_effect_hub_subscribe() {
        let mut file = make_file("src/events/hub.ts", Language::TypeScript);
        file.imports = vec![make_import_with_names("effect", vec!["Hub"])];
        file.call_sites = vec![make_call("Hub.subscribe", Some("listenForEvents"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "listenForEvents"
                && e.entrypoint_type == EntrypointType::EventHandler));
    }

    #[test]
    fn test_detect_effect_stream_subpath_import() {
        let mut file = make_file("src/stream.ts", Language::TypeScript);
        file.imports = vec![make_import("effect/Stream")];
        file.call_sites = vec![make_call("Stream.runDrain", Some("drainEvents"))];
        let result = detect_entrypoints(&[file]);
        assert!(result.iter().any(
            |e| e.symbol == "drainEvents" && e.entrypoint_type == EntrypointType::EventHandler
        ));
    }

    #[test]
    fn test_no_effect_event_without_import() {
        let mut file = make_file("src/stream.ts", Language::TypeScript);
        file.call_sites = vec![make_call("Stream.run", Some("noImport"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .all(|e| e.entrypoint_type != EntrypointType::EventHandler));
    }

    // ========================================================================
    // Effect.ts service detection
    // ========================================================================

    #[test]
    fn test_detect_effect_service() {
        let mut file = make_file("src/services/user.ts", Language::TypeScript);
        file.imports = vec![make_import_with_names("effect", vec!["Effect", "Context"])];
        file.call_sites = vec![make_call("Effect.Service", Some("UserService"))];
        let result = detect_entrypoints(&[file]);
        assert!(result.iter().any(
            |e| e.symbol == "UserService" && e.entrypoint_type == EntrypointType::EffectService
        ));
    }

    #[test]
    fn test_detect_context_tag() {
        let mut file = make_file("src/services/db.ts", Language::TypeScript);
        file.imports = vec![make_import_with_names("effect", vec!["Context"])];
        file.call_sites = vec![make_call("Context.Tag", Some("DatabaseService"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "DatabaseService"
                && e.entrypoint_type == EntrypointType::EffectService));
    }

    #[test]
    fn test_detect_context_generic_tag() {
        let mut file = make_file("src/services/config.ts", Language::TypeScript);
        file.imports = vec![make_import("effect/Context")];
        file.call_sites = vec![make_call("Context.GenericTag", Some("ConfigService"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "ConfigService"
                && e.entrypoint_type == EntrypointType::EffectService));
    }

    #[test]
    fn test_detect_layer_succeed() {
        let mut file = make_file("src/layers/live.ts", Language::TypeScript);
        file.imports = vec![make_import_with_names("effect", vec!["Layer"])];
        file.call_sites = vec![make_call("Layer.succeed", Some("LiveLayer"))];
        let result = detect_entrypoints(&[file]);
        assert!(
            result
                .iter()
                .any(|e| e.symbol == "LiveLayer"
                    && e.entrypoint_type == EntrypointType::EffectService)
        );
    }

    #[test]
    fn test_detect_layer_effect() {
        let mut file = make_file("src/layers/db.ts", Language::TypeScript);
        file.imports = vec![make_import("effect/Layer")];
        file.call_sites = vec![make_call("Layer.effect", Some("DbLayer"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "DbLayer" && e.entrypoint_type == EntrypointType::EffectService));
    }

    #[test]
    fn test_detect_layer_scoped() {
        let mut file = make_file("src/layers/connection.ts", Language::TypeScript);
        file.imports = vec![make_import_with_names("effect", vec!["Layer", "Effect"])];
        file.call_sites = vec![make_call("Layer.scoped", Some("ConnectionLayer"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "ConnectionLayer"
                && e.entrypoint_type == EntrypointType::EffectService));
    }

    #[test]
    fn test_no_effect_service_without_import() {
        let mut file = make_file("src/services/user.ts", Language::TypeScript);
        file.call_sites = vec![make_call("Effect.Service", Some("UserService"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .all(|e| e.entrypoint_type != EntrypointType::EffectService));
    }

    // ========================================================================
    // Effect.ts edge cases
    // ========================================================================

    #[test]
    fn test_effect_ts_python_file_ignored() {
        let mut file = make_file("src/services/user.py", Language::Python);
        file.imports = vec![make_import_with_names("effect", vec!["Effect"])];
        file.call_sites = vec![make_call("Effect.Service", Some("UserService"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .all(|e| e.entrypoint_type != EntrypointType::EffectService));
    }

    #[test]
    fn test_effect_multiple_entrypoint_types() {
        let mut file = make_file("src/api/server.ts", Language::TypeScript);
        file.imports = vec![
            make_import_with_names("@effect/platform", vec!["HttpApiEndpoint"]),
            make_import_with_names("effect", vec!["Effect", "Layer"]),
        ];
        file.call_sites = vec![
            make_call("HttpApiEndpoint.get", Some("getEndpoint")),
            make_call("Layer.succeed", Some("ApiLayer")),
        ];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.entrypoint_type == EntrypointType::HttpRoute));
        assert!(result
            .iter()
            .any(|e| e.entrypoint_type == EntrypointType::EffectService));
    }

    #[test]
    fn test_effect_platform_node_import() {
        let mut file = make_file("src/server.ts", Language::TypeScript);
        file.imports = vec![make_import_with_names(
            "@effect/platform-node",
            vec!["HttpServer"],
        )];
        file.call_sites = vec![make_call("HttpRouter.get", Some("serveApp"))];
        let result = detect_entrypoints(&[file]);
        assert!(result
            .iter()
            .any(|e| e.symbol == "serveApp" && e.entrypoint_type == EntrypointType::HttpRoute));
    }

    #[test]
    fn test_effect_deduplication_with_regular_detection() {
        // A file detected as test by both path-based and Effect.ts vitest detection
        let mut file = make_file("src/auth.test.ts", Language::TypeScript);
        file.imports = vec![make_import("@effect/vitest")];
        file.call_sites = vec![make_call("it.effect", Some("describe"))];
        let result = detect_entrypoints(&[file]);
        // Should deduplicate — same (file, symbol) pair
        let test_entries: Vec<_> = result
            .iter()
            .filter(|e| e.file == "src/auth.test.ts" && e.symbol == "describe")
            .collect();
        assert_eq!(test_entries.len(), 1);
    }

    // ========================================================================
    // Edge cases (existing + extended)
    // ========================================================================

    #[test]
    fn test_unknown_language_no_entrypoints() {
        let file = make_file("main.go", Language::Unknown);
        let result = detect_entrypoints(&[file]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_file_stem_extraction() {
        assert_eq!(file_stem("src/utils/format.ts"), "format");
        assert_eq!(file_stem("main.py"), "main");
        assert_eq!(file_stem("Makefile"), "Makefile");
    }

    // ========================================================================
    // Path detection helpers
    // ========================================================================

    #[test]
    fn test_is_test_path_variants() {
        assert!(is_test_path("src/utils.test.ts"));
        assert!(is_test_path("src/utils.spec.js"));
        assert!(is_test_path("__tests__/App.test.tsx"));
        assert!(is_test_path("tests/test_utils.py"));
        assert!(is_test_path("test/integration.py"));
        assert!(is_test_path("src/auth_test.py"));
        assert!(!is_test_path("src/utils.ts"));
        assert!(!is_test_path("src/testing-utils.ts"));
    }

    #[test]
    fn test_is_nextjs_route_file() {
        assert!(is_nextjs_route_file("src/app/api/users/route.ts"));
        assert!(is_nextjs_route_file("app/api/route.ts"));
        assert!(!is_nextjs_route_file("src/app/api/users/page.ts"));
        assert!(!is_nextjs_route_file("src/routes/users.ts"));
    }

    #[test]
    fn test_is_worker_path() {
        assert!(is_worker_path("src/workers/email.ts"));
        assert!(is_worker_path("src/jobs/cleanup.py"));
        assert!(is_worker_path("src/email_worker.ts"));
        assert!(!is_worker_path("src/services/email.ts"));
    }

    // =======================================================================
    // IR-based entrypoint parity tests
    // =======================================================================

    mod ir_parity {
        use super::*;
        use crate::ast;
        use crate::ir::IrFile;

        /// Helper: detect entrypoints via both paths and compare.
        fn detect_both(files: &[(&str, &str)]) -> (Vec<Entrypoint>, Vec<Entrypoint>) {
            let parsed: Vec<ParsedFile> = files
                .iter()
                .map(|(path, source)| ast::parse_file(path, source).unwrap())
                .collect();
            let ir_files: Vec<IrFile> = parsed.iter().map(IrFile::from_parsed_file).collect();

            let from_parsed = detect_entrypoints(&parsed);
            let from_ir = detect_entrypoints_ir(&ir_files);
            (from_parsed, from_ir)
        }

        #[test]
        fn test_ir_parity_test_file() {
            let (ep, ei) = detect_both(&[(
                "src/utils.test.ts",
                r#"
function test_validate() {}
function test_sanitize() {}
"#,
            )]);

            assert_eq!(ep.len(), ei.len(), "entrypoint count should match");
            for (a, b) in ep.iter().zip(ei.iter()) {
                assert_eq!(a.file, b.file);
                assert_eq!(a.symbol, b.symbol);
                assert_eq!(a.entrypoint_type, b.entrypoint_type);
            }
        }

        #[test]
        fn test_ir_parity_express_route() {
            let (ep, ei) = detect_both(&[(
                "src/routes/users.ts",
                r#"
import express from 'express';
const router = express.Router();
function getUsers() {}
router.get('/users', getUsers);
"#,
            )]);

            assert_eq!(
                ep.len(),
                ei.len(),
                "Express route entrypoint count should match"
            );
            for (a, b) in ep.iter().zip(ei.iter()) {
                assert_eq!(a.file, b.file);
                assert_eq!(a.entrypoint_type, b.entrypoint_type);
            }
        }

        #[test]
        fn test_ir_parity_flask_route() {
            let (ep, ei) = detect_both(&[(
                "app/views.py",
                r#"
from flask import Flask
app = Flask(__name__)

def list_users():
    pass
app.route('/users')(list_users)
"#,
            )]);

            assert_eq!(ep.len(), ei.len());
        }

        #[test]
        fn test_ir_parity_nextjs_route() {
            let (ep, ei) = detect_both(&[(
                "src/app/api/users/route.ts",
                r#"
export function GET(request: Request) {
    return Response.json({});
}
export function POST(request: Request) {
    return Response.json({});
}
"#,
            )]);

            assert_eq!(
                ep.len(),
                ei.len(),
                "Next.js route entrypoint count should match"
            );
            for (a, b) in ep.iter().zip(ei.iter()) {
                assert_eq!(a.file, b.file);
                assert_eq!(a.symbol, b.symbol);
            }
        }

        #[test]
        fn test_ir_parity_cli_command() {
            let (ep, ei) = detect_both(&[(
                "src/cli.py",
                r#"
import click

def main():
    pass

click.command()(main)
"#,
            )]);

            assert_eq!(ep.len(), ei.len());
        }

        #[test]
        fn test_ir_parity_no_entrypoints() {
            let (ep, ei) = detect_both(&[(
                "src/utils.ts",
                r#"
export function validate(data: any) { return data; }
export function sanitize(data: any) { return data; }
"#,
            )]);

            assert_eq!(ep.len(), ei.len(), "no entrypoints should be found");
            assert!(ep.is_empty());
        }

        #[test]
        fn test_ir_parity_multiple_files() {
            let (ep, ei) = detect_both(&[
                (
                    "src/app/api/users/route.ts",
                    r#"
export function GET() { return Response.json([]); }
"#,
                ),
                (
                    "src/utils.test.ts",
                    r#"
function test_something() {}
"#,
                ),
                (
                    "src/services/user.ts",
                    r#"
export function createUser(data: any) {}
"#,
                ),
            ]);

            assert_eq!(
                ep.len(),
                ei.len(),
                "multi-file entrypoint count should match"
            );
            for (a, b) in ep.iter().zip(ei.iter()) {
                assert_eq!(a.file, b.file);
                assert_eq!(a.symbol, b.symbol);
                assert_eq!(a.entrypoint_type, b.entrypoint_type);
            }
        }

        #[test]
        fn test_ir_parity_empty() {
            let from_parsed = detect_entrypoints(&[]);
            let from_ir = detect_entrypoints_ir(&[]);
            assert_eq!(from_parsed.len(), from_ir.len());
            assert!(from_ir.is_empty());
        }

        #[test]
        fn test_ir_parity_effect_ts_service() {
            let (ep, ei) = detect_both(&[(
                "src/services/user.ts",
                r#"
import { Effect, Context } from 'effect';
function UserService() {}
Effect.Service(UserService);
"#,
            )]);

            assert_eq!(ep.len(), ei.len());
        }

        #[test]
        fn test_ir_parity_queue_consumer() {
            let (ep, ei) = detect_both(&[(
                "src/workers/email.ts",
                r#"
import { Queue } from 'bullmq';
function processEmail() {}
queue.process(processEmail);
"#,
            )]);

            assert_eq!(ep.len(), ei.len());
        }
    }

    // ========================================================================
    // Phase 3: Path-based entrypoint detection (spec §1.4)
    // ========================================================================

    #[test]
    fn test_ts_routes_dir_with_express() {
        let mut file = make_file("src/routes/users.ts", Language::TypeScript);
        file.imports = vec![make_import("express")];
        file.definitions = vec![make_def("getUsers", SymbolKind::Function)];
        let result = detect_entrypoints(&[file]);
        assert!(
            result
                .iter()
                .any(|e| e.entrypoint_type == EntrypointType::HttpRoute),
            "TS file in /routes/ with express import should be detected as HTTP entrypoint"
        );
    }

    #[test]
    fn test_ts_routes_dir_no_import_strong_path() {
        let mut file = make_file("src/routes/users.ts", Language::TypeScript);
        // No framework import — but /routes/ is a strong path signal
        file.definitions = vec![make_def("getUsers", SymbolKind::Function)];
        file.exports = vec![make_export("getUsers", false)];
        let result = detect_entrypoints(&[file]);
        assert!(
            result
                .iter()
                .any(|e| e.entrypoint_type == EntrypointType::HttpRoute),
            "TS file in /routes/ should be detected as entrypoint (strong path)"
        );
    }

    #[test]
    fn test_ts_controller_suffix_nestjs() {
        let mut file = make_file("src/billing.controller.ts", Language::TypeScript);
        file.imports = vec![make_import("@nestjs/common")];
        file.definitions = vec![make_def("create", SymbolKind::Function)];
        let result = detect_entrypoints(&[file]);
        assert!(
            result
                .iter()
                .any(|e| e.entrypoint_type == EntrypointType::HttpRoute),
            "*.controller.ts with @nestjs/common import should be detected"
        );
    }

    #[test]
    fn test_ts_controller_suffix_strong_path() {
        let mut file = make_file("src/billing.controller.ts", Language::TypeScript);
        // No import — strong path (controller suffix)
        file.definitions = vec![make_def("create", SymbolKind::Function)];
        file.exports = vec![make_export("create", false)];
        let result = detect_entrypoints(&[file]);
        assert!(
            result
                .iter()
                .any(|e| e.entrypoint_type == EntrypointType::HttpRoute),
            "*.controller.ts should be detected as entrypoint (strong path)"
        );
    }

    #[test]
    fn test_ts_entrypoints_in_name() {
        let mut file = make_file("src/command-entrypoints.ts", Language::TypeScript);
        file.definitions = vec![make_def("deploy", SymbolKind::Function)];
        file.exports = vec![make_export("deploy", false)];
        let result = detect_entrypoints(&[file]);
        assert!(
            !result.is_empty(),
            "File with 'entrypoints' in name should be detected (strong path)"
        );
    }

    #[test]
    fn test_go_handlers_dir() {
        let mut file = make_file("internal/handlers/auth.go", Language::Go);
        file.imports = vec![make_import("net/http")];
        file.definitions = vec![make_def("HandleAuth", SymbolKind::Function)];
        let result = detect_entrypoints(&[file]);
        assert!(
            result
                .iter()
                .any(|e| e.entrypoint_type == EntrypointType::HttpRoute),
            "Go file in /handlers/ with net/http import should be detected"
        );
    }

    #[test]
    fn test_python_views_flask() {
        let mut file = make_file("app/views/dashboard.py", Language::Python);
        file.imports = vec![make_import("flask")];
        file.definitions = vec![make_def("index", SymbolKind::Function)];
        let result = detect_entrypoints(&[file]);
        assert!(
            result
                .iter()
                .any(|e| e.entrypoint_type == EntrypointType::HttpRoute),
            "Python file in /views/ with flask import should be detected"
        );
    }

    #[test]
    fn test_java_controller_spring() {
        let mut file = make_file("com/api/controllers/UserController.java", Language::Java);
        file.imports = vec![make_import(
            "org.springframework.web.bind.annotation.RestController",
        )];
        file.definitions = vec![make_def("getUser", SymbolKind::Function)];
        let result = detect_entrypoints(&[file]);
        assert!(
            result
                .iter()
                .any(|e| e.entrypoint_type == EntrypointType::HttpRoute),
            "Java controller in /controllers/ with Spring import should be detected"
        );
    }

    #[test]
    fn test_rust_handlers_axum() {
        let mut file = make_file("src/handlers/auth.rs", Language::Rust);
        file.imports = vec![make_import("axum")];
        file.definitions = vec![make_def("login", SymbolKind::Function)];
        let result = detect_entrypoints(&[file]);
        assert!(
            result
                .iter()
                .any(|e| e.entrypoint_type == EntrypointType::HttpRoute),
            "Rust file in /handlers/ with axum import should be detected"
        );
    }

    #[test]
    fn test_no_false_positive_utils() {
        let mut file = make_file("src/utils/helpers.ts", Language::TypeScript);
        file.definitions = vec![make_def("formatDate", SymbolKind::Function)];
        let result = detect_entrypoints(&[file]);
        assert!(
            result.is_empty(),
            "src/utils/helpers.ts should NOT be detected as entrypoint"
        );
    }

    #[test]
    fn test_no_false_positive_api_types() {
        let mut file = make_file("src/api/types.ts", Language::TypeScript);
        // No framework import, not a strong path
        file.definitions = vec![make_def("UserType", SymbolKind::TypeAlias)];
        let result = detect_entrypoints(&[file]);
        assert!(
            result.is_empty(),
            "src/api/types.ts without framework import should NOT be detected"
        );
    }

    #[test]
    fn test_cli_commands_dir_with_commander() {
        let mut file = make_file("src/commands/deploy.ts", Language::TypeScript);
        file.imports = vec![make_import("commander")];
        file.definitions = vec![make_def("deploy", SymbolKind::Function)];
        let result = detect_entrypoints(&[file]);
        assert!(
            result
                .iter()
                .any(|e| e.entrypoint_type == EntrypointType::CliCommand),
            "TS file in /commands/ with commander import should be detected as CLI"
        );
    }

    #[test]
    fn test_cli_commands_dir_strong_path() {
        let mut file = make_file("src/commands/migrate.ts", Language::TypeScript);
        file.definitions = vec![make_def("migrate", SymbolKind::Function)];
        file.exports = vec![make_export("migrate", false)];
        let result = detect_entrypoints(&[file]);
        assert!(
            !result.is_empty(),
            "/commands/ dir should detect entrypoints (strong path)"
        );
    }

    // ========================================================================
    // Path helper unit tests
    // ========================================================================

    #[test]
    fn test_is_route_handler_path() {
        assert!(is_route_handler_path("src/routes/users.ts"));
        assert!(is_route_handler_path("src/handlers/auth.go"));
        assert!(is_route_handler_path("src/controllers/billing.ts"));
        assert!(is_route_handler_path("src/endpoints/api.ts"));
        assert!(is_route_handler_path("src/billing.controller.ts"));
        assert!(is_route_handler_path("src/users.route.ts"));
        assert!(!is_route_handler_path("src/utils/helpers.ts"));
        assert!(!is_route_handler_path("src/models/user.ts"));
    }

    #[test]
    fn test_is_strong_route_handler_path() {
        assert!(is_strong_route_handler_path("src/routes/users.ts"));
        assert!(is_strong_route_handler_path("src/handlers/auth.go"));
        assert!(is_strong_route_handler_path("src/controllers/billing.ts"));
        assert!(is_strong_route_handler_path("src/billing.controller.ts"));
        assert!(is_strong_route_handler_path("src/command-entrypoints.ts"));
        assert!(!is_strong_route_handler_path("src/utils/helpers.ts"));
        assert!(!is_strong_route_handler_path("src/api/types.ts"));
    }

    #[test]
    fn test_is_cli_command_path() {
        assert!(is_cli_command_path("src/commands/deploy.ts"));
        assert!(is_cli_command_path("src/cmd/run.go"));
        assert!(is_cli_command_path("src/cli/main.ts"));
        assert!(is_cli_command_path("src/deploy.command.ts"));
        assert!(!is_cli_command_path("src/utils/helpers.ts"));
    }

    #[test]
    fn test_framework_import_helpers() {
        // JS web
        assert!(is_js_web_framework_import(&make_import("express")));
        assert!(is_js_web_framework_import(&make_import("@nestjs/common")));
        assert!(is_js_web_framework_import(&make_import("hono")));
        assert!(!is_js_web_framework_import(&make_import("lodash")));

        // Go web
        assert!(is_go_web_framework_import(&make_import("net/http")));
        assert!(is_go_web_framework_import(&make_import(
            "github.com/gin-gonic/gin"
        )));
        assert!(!is_go_web_framework_import(&make_import("fmt")));

        // Rust web
        assert!(is_rust_web_framework_import(&make_import("axum")));
        assert!(is_rust_web_framework_import(&make_import("actix_web")));
        assert!(!is_rust_web_framework_import(&make_import("serde")));

        // Java web
        assert!(is_java_web_framework_import(&make_import(
            "org.springframework.web.bind.annotation.RestController"
        )));
        assert!(!is_java_web_framework_import(&make_import(
            "java.util.List"
        )));

        // JS CLI
        assert!(is_js_cli_framework_import(&make_import("commander")));
        assert!(is_js_cli_framework_import(&make_import("yargs")));
        assert!(is_js_cli_framework_import(&make_import("@effect/cli")));
        assert!(!is_js_cli_framework_import(&make_import("express")));
    }

    // ===================================================================
    // Property-based tests for path detection helpers (spec §1)
    // ===================================================================

    mod proptests_path {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Strong route handler paths are always also regular route handler paths.
            #[test]
            fn prop_strong_route_implies_regular(
                dir in prop_oneof![
                    Just("routes"),
                    Just("handlers"),
                    Just("controllers"),
                    Just("endpoints"),
                ],
                name in "[a-z]{3,10}",
                ext in prop_oneof![Just(".ts"), Just(".js"), Just(".py"), Just(".go")],
            ) {
                let path = format!("src/{}/{}{}", dir, name, ext);
                if is_strong_route_handler_path(&path) {
                    prop_assert!(
                        is_route_handler_path(&path),
                        "strong path '{}' must also be a regular route path", path,
                    );
                }
            }

            /// Route handler path detection is case-insensitive.
            #[test]
            fn prop_route_handler_case_insensitive(
                dir in prop_oneof![
                    Just("routes"),
                    Just("handlers"),
                    Just("controllers"),
                ],
                name in "[a-z]{3,10}",
            ) {
                let lower = format!("src/{}/{}.ts", dir, name);
                let upper = format!("SRC/{}/{}.TS", dir.to_uppercase(), name.to_uppercase());
                prop_assert_eq!(
                    is_route_handler_path(&lower),
                    is_route_handler_path(&upper),
                    "detection should be case-insensitive: '{}' vs '{}'", lower, upper,
                );
            }

            /// Files in plain src/ directories (no route/handler/controller pattern) are not
            /// detected as route handlers.
            #[test]
            fn prop_plain_src_not_route(name in "[a-z]{3,15}") {
                let path = format!("src/{}.ts", name);
                // Only assert when name doesn't accidentally contain a pattern keyword
                prop_assume!(
                    !name.contains("route") && !name.contains("handler")
                    && !name.contains("controller") && !name.contains("endpoint")
                    && !name.contains("entrypoint")
                );
                prop_assert!(
                    !is_route_handler_path(&path),
                    "'{}' should not be a route handler path", path,
                );
            }

            /// CLI command path detection: files in /commands/ are always detected.
            #[test]
            fn prop_cli_commands_dir_always_detected(
                name in "[a-z]{3,10}",
                ext in prop_oneof![Just(".ts"), Just(".go"), Just(".py"), Just(".rs")],
            ) {
                let path = format!("src/commands/{}{}", name, ext);
                prop_assert!(
                    is_cli_command_path(&path),
                    "'{}' should be a CLI command path", path,
                );
            }

            /// CLI and route directory patterns don't overlap (commands/ is CLI, routes/ is HTTP).
            #[test]
            fn prop_cli_route_dirs_disjoint(name in "[a-z]{3,10}") {
                let cli_path = format!("src/commands/{}.ts", name);
                let route_path = format!("src/routes/{}.ts", name);

                // Guard: name doesn't contain keywords from the other domain
                prop_assume!(
                    !name.contains("route") && !name.contains("handler")
                    && !name.contains("controller") && !name.contains("endpoint")
                );
                prop_assume!(
                    !name.contains("command") && !name.contains("cli")
                );

                prop_assert!(is_cli_command_path(&cli_path), "commands/ should be CLI");
                prop_assert!(!is_route_handler_path(&cli_path), "commands/ should NOT be HTTP");
                prop_assert!(is_route_handler_path(&route_path), "routes/ should be HTTP");
                prop_assert!(!is_cli_command_path(&route_path), "routes/ should NOT be CLI");
            }

            /// has_filename_pattern requires dot delimiters — partial substring matches don't count.
            #[test]
            fn prop_filename_pattern_requires_dots(
                prefix in "[a-z]{2,8}",
                pattern in prop_oneof![
                    Just("controller"),
                    Just("route"),
                    Just("handler"),
                ],
                ext in prop_oneof![Just(".ts"), Just(".js")],
            ) {
                // With dots: "prefix.pattern.ext" → should match
                let dotted = format!("src/{}.{}{}", prefix, pattern, ext);
                prop_assert!(
                    has_filename_pattern(&dotted.to_lowercase(), pattern),
                    "'{}' with dot-delimited pattern should match", dotted,
                );

                // Without dots: "prefixpatternext" → should NOT match
                let no_dots = format!("src/{}{}{}", prefix, pattern, ext);
                // Only assert if the concatenation doesn't accidentally create a dot pattern
                if !no_dots.to_lowercase().contains(&format!(".{}.", pattern)) {
                    prop_assert!(
                        !has_filename_pattern(&no_dots.to_lowercase(), pattern),
                        "'{}' without dot delimiters should not match", no_dots,
                    );
                }
            }

            /// is_route_handler_path is deterministic.
            #[test]
            fn prop_route_handler_deterministic(path in "[a-z/._]{1,50}") {
                let r1 = is_route_handler_path(&path);
                let r2 = is_route_handler_path(&path);
                prop_assert_eq!(r1, r2);
            }

            /// is_cli_command_path is deterministic.
            #[test]
            fn prop_cli_command_deterministic(path in "[a-z/._]{1,50}") {
                let r1 = is_cli_command_path(&path);
                let r2 = is_cli_command_path(&path);
                prop_assert_eq!(r1, r2);
            }
        }
    }

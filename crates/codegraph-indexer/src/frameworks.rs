//! Per-language framework route and config convention detectors.

use std::path::Path;

#[allow(unused_imports)]
use crate::*;

pub(crate) fn file_framework_configs(label: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    let file_name = Path::new(label)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(label);
    let lower_label = label.to_ascii_lowercase();
    let lower_name = file_name.to_ascii_lowercase();

    if lower_name == "settings.py" {
        configs.push(framework_config(
            "django",
            format!("django settings:{label}"),
            "settings_module",
            Some(label.to_string()),
            1,
        ));
    }

    for (prefix, framework, kind) in [
        ("next.config.", "nextjs", "config_file"),
        ("vite.config.", "vite", "config_file"),
        ("nuxt.config.", "nuxt", "config_file"),
        ("webpack.config.", "webpack", "config_file"),
        ("svelte.config.", "sveltekit", "config_file"),
    ] {
        if lower_name.starts_with(prefix) {
            configs.push(framework_config(
                framework,
                format!("{framework} config:{label}"),
                kind,
                Some(label.to_string()),
                1,
            ));
        }
    }

    if lower_label.starts_with("config/") && lower_label.ends_with(".php") {
        configs.push(framework_config(
            "laravel",
            format!("laravel config:{label}"),
            "config_file",
            Some(label.to_string()),
            1,
        ));
    }

    if lower_label.starts_with("config/packages/")
        && (lower_label.ends_with(".yaml")
            || lower_label.ends_with(".yml")
            || lower_label.ends_with(".xml")
            || lower_label.ends_with(".php"))
    {
        configs.push(framework_config(
            "symfony",
            format!("symfony config:{label}"),
            "config_file",
            Some(label.to_string()),
            1,
        ));
    }

    configs
}

pub(crate) fn python_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains(".config.from_pyfile(")
            && let Some(value) = first_quoted_value(trimmed)
        {
            configs.push(framework_config(
                "flask",
                format!("flask config:{value}"),
                "config_file",
                Some(value),
                line_number,
            ));
        }

        if lower.contains(".config.from_object(")
            && let Some(value) = first_quoted_value(trimmed)
        {
            configs.push(framework_config(
                "flask",
                format!("flask config object:{value}"),
                "config_object",
                Some(value),
                line_number,
            ));
        }

        if lower.contains("settingsconfigdict(")
            && lower.contains("env_file")
            && let Some(value) = first_quoted_value(trimmed)
        {
            configs.push(framework_config(
                "pydantic",
                format!("pydantic env file:{value}"),
                "env_file",
                Some(value),
                line_number,
            ));
        }

        if lower.starts_with("class ")
            && lower.contains("basesettings")
            && let Some(class_name) = trimmed
                .strip_prefix("class ")
                .and_then(|rest| rest.split_once('(').map(|(name, _)| name.trim()))
                .filter(|name| !name.is_empty())
        {
            configs.push(framework_config(
                "pydantic",
                format!("pydantic settings:{class_name}"),
                "settings_class",
                Some(class_name.to_string()),
                line_number,
            ));
        }
    }
    configs
}

pub(crate) fn js_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains("dotenv.config(") {
            let value = first_quoted_value(trimmed).unwrap_or_else(|| ".env".to_string());
            configs.push(framework_config(
                "dotenv",
                format!("dotenv config:{value}"),
                "env_file",
                Some(value),
                line_number,
            ));
        }

        if let Some(setting) = express_setting(trimmed) {
            configs.push(framework_config(
                "express",
                format!("express setting:{setting}"),
                "setting",
                Some(setting),
                line_number,
            ));
        }
    }
    configs
}

pub(crate) fn rust_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains("dotenv") && lower.contains("dotenv(") {
            configs.push(framework_config(
                "dotenv",
                "dotenv config:.env".to_string(),
                "env_file",
                Some(".env".to_string()),
                line_number,
            ));
        }

        if lower.contains("environment::with_prefix(")
            && let Some(value) = first_quoted_value_after(trimmed, "Environment::with_prefix(")
        {
            configs.push(framework_config(
                "config-rs",
                format!("config-rs env prefix:{value}"),
                "env_prefix",
                Some(value),
                line_number,
            ));
        }
    }
    configs
}

pub(crate) fn go_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains("viper.setconfigname(")
            && let Some(value) = first_quoted_value_after(trimmed, "SetConfigName(")
        {
            configs.push(framework_config(
                "viper",
                format!("viper config:{value}"),
                "config_name",
                Some(value),
                line_number,
            ));
        }

        if lower.contains("viper.addconfigpath(")
            && let Some(value) = first_quoted_value_after(trimmed, "AddConfigPath(")
        {
            configs.push(framework_config(
                "viper",
                format!("viper config path:{value}"),
                "config_path",
                Some(value),
                line_number,
            ));
        }

        if lower.contains("godotenv.load(") {
            let value =
                first_quoted_value_after(trimmed, "Load(").unwrap_or_else(|| ".env".to_string());
            configs.push(framework_config(
                "godotenv",
                format!("godotenv config:{value}"),
                "env_file",
                Some(value),
                line_number,
            ));
        }
    }
    configs
}

pub(crate) fn php_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains("config(")
            && let Some(value) = first_quoted_value_after(trimmed, "config(")
        {
            configs.push(framework_config(
                "laravel",
                format!("laravel config key:{value}"),
                "config_key",
                Some(value),
                line_number,
            ));
        }

        if lower.contains("->configure(")
            && let Some(value) = first_quoted_value_after(trimmed, "->configure(")
        {
            configs.push(framework_config(
                "lumen",
                format!("lumen config:{value}"),
                "config_file",
                Some(value),
                line_number,
            ));
        }
    }
    configs
}

pub(crate) fn bash_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        if let Some(value) = sourced_shell_config(trimmed) {
            configs.push(framework_config(
                "shell",
                format!("shell config:{value}"),
                "source_file",
                Some(value),
                line_number,
            ));
        }
    }
    configs
}

pub(crate) fn dart_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();

        for needle in [
            "rootBundle.loadString(",
            "rootBundle.load(",
            "AssetImage(",
            "Image.asset(",
            "SvgPicture.asset(",
        ] {
            if let Some(value) = first_quoted_value_after(trimmed, needle)
                && is_flutter_asset_path(&value)
            {
                configs.push(framework_config(
                    "flutter",
                    format!("flutter asset read:{value}"),
                    "flutter_asset_read",
                    Some(value),
                    line_number,
                ));
            }
        }
    }
    configs
}

pub(crate) fn framework_config(
    framework: &str,
    label: String,
    config_kind: &str,
    value: Option<String>,
    line: u32,
) -> FrameworkConfig {
    FrameworkConfig {
        framework: framework.to_string(),
        label,
        config_kind: config_kind.to_string(),
        value,
        line,
    }
}

pub(crate) fn express_setting(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let setting_index = lower
        .find(".set(")
        .or_else(|| lower.find("app.set("))
        .or_else(|| lower.find("server.set("))?;
    let receiver = line[..setting_index]
        .rsplit(|character: char| {
            !(character.is_ascii_alphanumeric() || matches!(character, '_' | '$'))
        })
        .next()
        .unwrap_or("")
        .trim_start_matches('$');
    if !["app", "server", "router"].iter().any(|allowed| {
        receiver.eq_ignore_ascii_case(allowed)
            || lower[setting_index..].starts_with(&format!("{allowed}.set("))
    }) {
        return None;
    }
    first_quoted_value(line)
}

pub(crate) fn sourced_shell_config(line: &str) -> Option<String> {
    let without_comment = line.split('#').next().unwrap_or("").trim();
    let rest = without_comment
        .strip_prefix("source ")
        .or_else(|| without_comment.strip_prefix(". "))?
        .trim();
    let value = rest
        .split_whitespace()
        .next()
        .map(|value| value.trim_matches(['"', '\'']).to_string())?;
    if value.contains("env") || value.contains("config") || value.ends_with(".conf") {
        Some(value)
    } else {
        None
    }
}

pub(crate) fn python_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    let mut routes = django_url_routes(source);
    let mut pending = Vec::new();
    let framework = python_route_framework(source);

    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.starts_with('@') {
            if let Some(mut route) = route_from_python_decorator(trimmed, line_number, framework) {
                route.handler = None;
                pending.push(route);
            }
            continue;
        }

        if let Some(function) = trimmed
            .strip_prefix("def ")
            .and_then(|rest| rest.split_once('(').map(|(name, _)| name.trim()))
            .filter(|name| !name.is_empty())
        {
            for mut route in pending.drain(..) {
                route.handler = Some(function.to_string());
                routes.push(route);
            }
        } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
            pending.clear();
        }
    }

    routes
}

/// Django states its routes in a URLconf rather than on the handler:
/// `path("", self.catalogue_view.as_view(), name="index")` and
/// `re_path(r"^ranges/(?P<slug>[\w-]+)/$", view, name="range")`, the
/// second of which is usually written across several lines. django-oscar
/// declares 193 of them and the graph held none.
fn django_url_routes(source: &str) -> Vec<FrameworkRoute> {
    if !(source.contains("from django") || source.contains("import django")) {
        return Vec::new();
    }
    let lines: Vec<&str> = source.lines().collect();
    let mut routes = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start().trim_start_matches('[').trim_start();
        let Some(rest) = ["path(", "re_path(", "url("]
            .iter()
            .find_map(|opener| trimmed.strip_prefix(opener))
        else {
            continue;
        };
        // The pattern is the first argument, on this line or the next one
        // when the call is written across several.
        let (path, handler_source) = match first_quoted_value(rest) {
            Some(path) => (path, rest.to_string()),
            None => {
                let Some(next) = lines.get(index + 1).map(|line| line.trim()) else {
                    continue;
                };
                let Some(path) = first_quoted_value(next) else {
                    continue;
                };
                let handler_line = lines.get(index + 2).map(|line| line.trim()).unwrap_or("");
                (path, format!("{next} {handler_line}"))
            }
        };
        routes.push(FrameworkRoute {
            framework: "django".to_string(),
            // A URLconf entry answers whatever method its view allows, and
            // the view is where that is written.
            method: "ROUTE".to_string(),
            path: normalize_django_route_path(&path),
            handler: django_route_handler(&handler_source).map(|(name, _)| name),
            handler_qualifier: django_route_handler(&handler_source)
                .and_then(|(_, qualifier)| qualifier),
            expanded: false,
            constrained: false,
            line: index as u32 + 1,
        });
    }
    routes
}

/// The view a URLconf entry points at: `self.detail_view.as_view()` is
/// `detail_view`, `views.IndexView.as_view()` is `IndexView`, and
/// `include("oscar.apps.basket.urls")` names another URLconf rather than a
/// view.
fn django_route_handler(rest: &str) -> Option<(String, Option<String>)> {
    let after_path = rest.split_once(',')?.1;
    let mut candidate = after_path.split(',').next()?.trim();
    // `path("sitemap.xml", views.index)` closes the call on the handler,
    // so the parenthesis the route opened comes back with it.
    while candidate.ends_with(')')
        && candidate.matches(')').count() > candidate.matches('(').count()
    {
        candidate = candidate[..candidate.len() - 1].trim_end();
    }
    // `self.detail_view.as_view()` names an attribute of the app config,
    // whose value is assigned somewhere else entirely: django-oscar writes
    // 124 of them, and claiming a function called `detail_view` is a guess
    // the syntax cannot make good on.
    if candidate.is_empty() || candidate.starts_with("include(") || candidate.starts_with("self.") {
        return None;
    }
    let written = candidate
        .trim_end_matches("()")
        .trim_end_matches(".as_view");
    let name = written.rsplit('.').next()?.trim();
    // `views.index` is `index` written under `views`, and what `views` is
    // depends on what the file imports.
    let qualifier = written
        .rsplit_once('.')
        .map(|(qualifier, _)| qualifier.trim().to_string())
        .filter(|qualifier| {
            !qualifier.is_empty()
                && qualifier
                    .chars()
                    .all(|character| character.is_alphanumeric() || character == '_')
        });
    (!name.is_empty()
        && name
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_'))
    .then(|| (name.to_string(), qualifier))
}

/// What a reader would call the path a URLconf states: Django writes it
/// without a leading slash, and a `re_path` writes a regular expression.
fn normalize_django_route_path(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        return "/".to_string();
    }
    if path.starts_with('^') || path.starts_with('/') {
        return path.to_string();
    }
    format!("/{path}")
}

/// Which Python web framework a file's routes belong to, as the file
/// itself says. `@app.get("/")` is the same line in Flask 2 and in
/// FastAPI, so the decorator cannot tell them apart, and reading it as
/// FastAPI filed 45 of flask's own routes under the wrong framework. What
/// a file imports can tell them apart; where it names neither, neither is
/// claimed.
fn python_route_framework(source: &str) -> &'static str {
    let flask = source.contains("import flask")
        || source.contains("from flask")
        || source.contains("import Flask");
    let fastapi = source.contains("import fastapi") || source.contains("from fastapi");
    match (flask, fastapi) {
        (true, false) => "flask",
        (false, true) => "fastapi",
        _ => "python-route",
    }
}

pub(crate) fn route_from_python_decorator(
    line: &str,
    line_number: u32,
    framework: &str,
) -> Option<FrameworkRoute> {
    let lower = line.to_ascii_lowercase();
    if !(lower.contains(".route(")
        || route_methods()
            .iter()
            .any(|method| lower.contains(&format!(".{}(", method.to_ascii_lowercase()))))
    {
        return None;
    }
    let path = first_quoted_value(line)?;
    let method = route_methods()
        .iter()
        .find(|method| lower.contains(&format!(".{}(", method.to_ascii_lowercase())))
        .copied()
        .or_else(|| method_from_python_route_methods(line))
        .unwrap_or("ROUTE")
        .to_string();
    Some(FrameworkRoute {
        framework: framework.to_string(),
        method,
        path,
        handler: None,
        handler_qualifier: None,
        expanded: false,
        constrained: false,
        line: line_number,
    })
}

pub(crate) fn method_from_python_route_methods(line: &str) -> Option<&'static str> {
    let lower = line.to_ascii_uppercase();
    route_methods()
        .iter()
        .find(|method| {
            lower.contains(&format!("\"{method}\"")) || lower.contains(&format!("'{method}'"))
        })
        .copied()
}

pub(crate) fn js_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            route_from_call_line(
                line,
                index as u32 + 1,
                "express",
                &["app", "router", "server", "routes"],
            )
        })
        .collect()
}

pub(crate) fn rust_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    let lines = source.lines().collect::<Vec<_>>();
    lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index as u32 + 1;
            let trimmed = line.trim();
            find_unquoted(trimmed, ".route(")?;
            let call = rust_route_call_window(&lines, index);
            let route_index = find_unquoted(&call, ".route(")?;
            let route_args = &call[route_index + ".route(".len()..];
            let path = first_quoted_value(route_args)?;
            let lower_args = route_args.to_ascii_lowercase();
            let method = route_methods()
                .iter()
                .find(|method| {
                    find_unquoted(&lower_args, &format!("{}(", method.to_ascii_lowercase()))
                        .is_some()
                })
                .copied()
                .unwrap_or("ROUTE")
                .to_string();
            let handler = handler_from_rust_route(route_args);
            Some(FrameworkRoute {
                framework: "axum".to_string(),
                method,
                path,
                handler,
                handler_qualifier: None,
                expanded: false,
                constrained: false,
                line: line_number,
            })
        })
        .collect()
}

pub(crate) fn go_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index as u32 + 1;
            let trimmed = line.trim();
            if trimmed.contains("HandleFunc(") {
                let path = first_quoted_value(trimmed)?;
                let handler = handler_after_first_comma(trimmed);
                return Some(FrameworkRoute {
                    framework: "net/http".to_string(),
                    method: "ROUTE".to_string(),
                    path,
                    handler,
                    handler_qualifier: None,
                    expanded: false,
                    constrained: false,
                    line: line_number,
                });
            }
            route_from_call_line(
                trimmed,
                line_number,
                "go-router",
                &["r", "router", "engine", "api", "group", "v1", "v2"],
            )
        })
        .collect()
}

/// `get '/hello' do ... end` is how Sinatra declares a route, and Rails
/// writes the same shape in `routes.rb`. The path has to look like one, so
/// a bare `get 'name'` in a helper is not mistaken for a route.
pub(crate) fn ruby_framework_routes(label: &str, source: &str) -> Vec<FrameworkRoute> {
    const METHODS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options"];
    // Rails states its routes in a file of its own, and `resources :users`
    // is seven of them.
    // Rails splits a large router across `config/routes/*.rb`, each drawn
    // from the main file: mastodon writes five of them, and only the main
    // one says `routes.draw`. A file in that directory is a routes file.
    let rails = source.contains("routes.draw")
        || label == "config/routes.rb"
        || label.contains("config/routes/")
        || label.ends_with("/config/routes.rb");
    let mut routes = Vec::new();
    // Each open block states a path segment and a module: `namespace
    // :api` is both `/api` and `Api::`, and `scope module: :v1` is a
    // module with no path of its own.
    let mut prefixes: Vec<String> = Vec::new();
    let mut modules: Vec<String> = Vec::new();
    // The controller each open block belongs to, when it is a resource:
    // `get :export` inside `resources :export_domain_allows do` is the
    // `export` action of that resource's controller, and the line names
    // neither.
    let mut controllers: Vec<Option<String>> = Vec::new();
    // `with_options only: [:index], concerns: :batch do` hands its options
    // to every call inside it: mastodon writes six such blocks, and
    // reading the resources inside them without the options claimed 20
    // routes it does not serve.
    let mut inherited_options: Vec<Option<String>> = Vec::new();
    // The depth a `concern` block opened at, while one is open.
    let mut concern_depth: Option<usize> = None;
    // How deep the innermost `constraints .. do` block sits, when one is
    // open: every route inside it states a condition of its own.
    let mut constraint_depth: Option<usize> = None;

    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if rails {
            // `namespace :admin do` puts everything inside it under /admin.
            if let Some(rest) = trimmed.strip_prefix("namespace ")
                && trimmed.ends_with(" do")
                && let Some(name) = ruby_symbol_name(rest)
            {
                modules.push(rails_module_name(&name));
                prefixes.push(name);
                controllers.push(None);
                inherited_options.push(None);
                continue;
            }
            if trimmed == "end" {
                prefixes.pop();
                modules.pop();
                controllers.pop();
                inherited_options.pop();
                if concern_depth.is_some_and(|depth| depth >= prefixes.len()) {
                    concern_depth = None;
                }
                if constraint_depth.is_some_and(|depth| depth >= prefixes.len()) {
                    constraint_depth = None;
                }
                continue;
            }
            let prefix = joined_route_prefix(&prefixes);
            if let Some(rest) = trimmed.strip_prefix("root ") {
                routes.push(FrameworkRoute {
                    framework: "rails".to_string(),
                    method: "GET".to_string(),
                    path: if prefix.is_empty() {
                        "/".to_string()
                    } else {
                        prefix.clone()
                    },
                    handler: rails_route_handler(rest),
                    handler_qualifier: rails_route_target(rest)
                        .and_then(|(_, controller)| controller)
                        .and_then(|controller| qualified_rails_controller(&modules, &controller)),
                    expanded: false,
                    constrained: false,
                    line: line_number,
                });
                continue;
            }
            // `concern :account_resources do .. end` states routes to be
            // mounted elsewhere, by whoever writes `concerns:`. Reading
            // them where they are written put mastodon's `/inbox` and
            // `/outbox` at the root of the site.
            if let Some(rest) = trimmed.strip_prefix("concern ")
                && trimmed.ends_with(" do")
                && ruby_symbol_name(rest).is_some()
            {
                concern_depth = Some(prefixes.len());
                prefixes.push(String::new());
                modules.push(String::new());
                controllers.push(None);
                inherited_options.push(None);
                continue;
            }
            if let Some(rest) = trimmed
                .strip_prefix("resources ")
                .or_else(|| trimmed.strip_prefix("resource "))
            {
                let singular = trimmed.starts_with("resource ");
                let with_options = rails_line_with_inherited_options(rest, &inherited_options);
                let rest = with_options.as_str();
                if let Some(name) = ruby_symbol_name(rest) {
                    // `resources :accounts, path: 'users', only: [:show]`
                    // states one route, not seven, and states it under
                    // `/users`. Mastodon writes 64 such lines, and reading
                    // them as the whole set invented routes it does not
                    // serve.
                    let segment = rails_option_value(rest, "path").unwrap_or_else(|| name.clone());
                    // A singular `resource :setup` is served by
                    // `SetupsController`: the resource is one, the
                    // controller is named for the set. mastodon writes 26
                    // of them and every route they declare pointed at a
                    // controller that does not exist.
                    let controller = rails_option_value(rest, "controller").unwrap_or_else(|| {
                        if singular {
                            rails_plural(&name)
                        } else {
                            name.clone()
                        }
                    });
                    // `namespace :api do namespace :v2 do resources
                    // :search` is `Api::V2::SearchController`, and
                    // mastodon declares one of those per version.
                    // `resource :preview, module: :terms_of_service` puts
                    // the controller one module deeper without changing
                    // the path: mastodon serves nine routes that way, and
                    // each pointed at a class that does not exist.
                    let controller = match rails_option_value(rest, "module") {
                        Some(module) => {
                            let mut nested = modules.clone();
                            nested.push(rails_module_name(&module));
                            qualified_rails_controller(&nested, &controller)
                        }
                        None => qualified_rails_controller(&modules, &controller),
                    };
                    if concern_depth.is_none() {
                        routes.extend(rails_resource_routes(
                            &prefix,
                            &segment,
                            controller.clone(),
                            singular,
                            &rails_resource_actions(rest),
                            line_number,
                        ));
                    }
                    // `resources :accounts do resources :statuses end`
                    // nests: the inner routes live under the outer one's
                    // member path.
                    if trimmed.ends_with(" do") {
                        modules.push(String::new());
                        controllers.push(controller.clone());
                        inherited_options.push(None);
                        let member = if singular {
                            segment.clone()
                        } else {
                            let key = rails_option_value(rest, "param")
                                .unwrap_or_else(|| "id".to_string());
                            format!("{segment}/:{}_{key}", rails_singular(&name))
                        };
                        prefixes.push(member);
                    }
                }
                continue;
            }
        }

        // Every `end` pops one entry, so every block that opens has to
        // push one: mastodon's `config/routes/api.rb` opens `member do`,
        // `collection do` and `scope module: :v1 do` between its
        // namespaces, and the stack drained until `/api/v1/accounts` read
        // as `/accounts`.
        if rails && opens_a_ruby_block(trimmed) {
            if constraint_depth.is_none() && trimmed.starts_with("constraints") {
                constraint_depth = Some(prefixes.len());
            }
            prefixes.push(rails_block_prefix(trimmed));
            modules.push(rails_block_module(trimmed));
            controllers.push(None);
            inherited_options.push(
                trimmed
                    .strip_prefix("with_options ")
                    .and_then(|rest| rest.strip_suffix(" do"))
                    .map(|options| options.trim().to_string()),
            );
        }

        let Some((method, rest)) = METHODS
            .iter()
            .find_map(|method| trimmed.strip_prefix(method).map(|rest| (*method, rest)))
        else {
            continue;
        };
        // `get '/x' do` and `get('/x') do` declare the same route: a call
        // is written with or without parentheses.
        if !rest.starts_with(char::is_whitespace) && !rest.starts_with('(') {
            continue;
        }
        // `with_options to: 'streaming#index' do get '/streaming' end`
        // says which action serves the routes inside it.
        let with_options = rails_line_with_inherited_options(rest, &inherited_options);
        let rest = with_options.as_str();
        // Rails names the path with a symbol as often as with a string:
        // `get :verify_credentials, to: 'credentials#show'` is the path
        // `verify_credentials`, and reading the first quoted value on the
        // line took the controller as the path.
        let before_target = rest.split("to:").next().unwrap_or(rest);
        // `get :export` inside a resource block is that resource's
        // `export` action: the line names the action and the block names
        // the controller. mastodon writes 208 routes that way, and each
        // of them reached no code at all.
        let action =
            ruby_symbol_name(before_target).filter(|_| first_quoted_value(before_target).is_none());
        let Some(path) = first_quoted_value(before_target)
            .or_else(|| action.clone().map(|name| format!("/{name}")))
            .or_else(|| first_quoted_value(rest))
        else {
            continue;
        };
        if !rails && !path.starts_with('/') {
            continue;
        }
        // Sinatra declares a route with the block that serves it: `get
        // '/' do .. end`. A request spec writes `get '/accounts'` with no
        // block at all, and mastodon's suite made 469 of those read as
        // routes the program serves. A brace is a block only where ruby
        // lets it be one -- after a closed argument list, `get('/') {` --
        // and never on `params: {`, which opens the hash a spec passes:
        // taking any brace on the line let 148 of mastodon's specs back in
        // as routes the program does not serve.
        let opens_a_block = trimmed.ends_with(" do")
            || trimmed
                .strip_suffix('{')
                .is_some_and(|head| head.trim_end().ends_with(')'));
        if !rails && !opens_a_block {
            continue;
        }
        // A route written inside a `concern` block is a template: it is
        // served wherever `concerns:` names it, not where it is written.
        // mastodon writes `post :approve` inside `concern :approvable`,
        // and reading it here served `/api/v1/admin/trends/approve`,
        // which the program does not.
        if concern_depth.is_some() {
            continue;
        }
        let prefix = joined_route_prefix(&prefixes);
        routes.push(FrameworkRoute {
            framework: if rails { "rails" } else { "sinatra" }.to_string(),
            method: method.to_ascii_uppercase(),
            path: format!("{prefix}/{}", path.trim_start_matches('/')),
            handler: rails_route_handler(rest).or_else(|| {
                action
                    .clone()
                    .filter(|_| controllers.iter().any(Option::is_some))
            }),
            handler_qualifier: rails_route_target(rest)
                .and_then(|(_, controller)| controller)
                .and_then(|controller| qualified_rails_controller(&modules, &controller))
                .or_else(|| {
                    action.as_ref().and_then(|_| {
                        controllers
                            .iter()
                            .rev()
                            .find_map(|controller| controller.clone())
                    })
                }),
            expanded: false,
            // `constraints:` states a condition the request has to meet, so
            // the same path can be declared beside itself: mastodon serves
            // `/invite/:invite_code` as JSON from the API and as HTML from
            // the registration form.
            constrained: rest.contains("constraints:") || constraint_depth.is_some(),
            line: line_number,
        });
    }

    routes
}

/// The name a Ruby symbol states: `:users` in `resources :users`.
pub(crate) fn ruby_symbol_name(rest: &str) -> Option<String> {
    let value = rest.trim().strip_prefix(':')?;
    let name: String = value
        .chars()
        .take_while(|character| character.is_alphanumeric() || *character == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// The action a Rails route points at: `to: "health#show"` is `show` in the
/// health controller.
fn rails_route_handler(rest: &str) -> Option<String> {
    rails_route_target(rest).map(|(action, _)| action)
}

/// The action a route names and the controller that serves it: `to:
/// 'accounts#show'` is `show` on `AccountsController`, and mastodon
/// declares 139 methods called `show`.
fn rails_route_target(rest: &str) -> Option<(String, Option<String>)> {
    let value = rest.split("to:").nth(1)?;
    let target = first_quoted_value(value)?;
    let (controller, action) = target.split_once('#')?;
    // `to: redirect { |_, request| "/authorize_interaction?#{..}" }` is a
    // block, not a controller: the string it holds has a `#` in it and
    // nothing that follows is a method name.
    let names_a_method = !action.is_empty()
        && action.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '?' | '!')
        });
    names_a_method.then(|| (action.to_string(), rails_controller_class(controller)))
}

/// The class Rails looks for: `follower_accounts` is
/// `FollowerAccountsController`, and `auth/registrations` is
/// `Auth::RegistrationsController` -- the path states the modules the
/// class sits in, exactly as a `namespace` block does. Mastodon declares
/// both `Auth::RegistrationsController` and
/// `Admin::Fasp::RegistrationsController`, and the name alone chose
/// neither.
fn rails_controller_class(controller: &str) -> Option<String> {
    let controller = controller.trim();
    if controller.is_empty() {
        return None;
    }
    let camelize = |name: &str| -> String {
        name.split('_')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut characters = part.chars();
                match characters.next() {
                    Some(first) => format!("{}{}", first.to_ascii_uppercase(), characters.as_str()),
                    None => String::new(),
                }
            })
            .collect()
    };
    let camel: String = controller
        .split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(camelize)
        .collect::<Vec<_>>()
        .join("::");
    // The name may already be a class: `rails_route_target` classifies
    // what `to:` names before the namespaces are joined onto it.
    (!camel.is_empty()).then(|| {
        if camel.ends_with("Controller") {
            camel
        } else {
            format!("{camel}Controller")
        }
    })
}

/// What `resources :users` declares: the seven routes Rails generates for
/// it, and six for a singular `resource` which has no id of its own.
/// What `only:` and `except:` say a resource declares. Rails writes them
/// as symbol lists -- `only: [:show]`, `except: :destroy` -- and without
/// them every resource reads as the whole set of seven.
#[derive(Default)]
struct RailsResourceActions {
    /// The actions `only:` names, when the line states it. `only: []`
    /// declares none of the seven -- mastodon writes `resources :users,
    /// only: [] do` and serves nothing at `/admin/users` -- which is not
    /// the same as a line that says nothing about actions.
    only: Option<Vec<String>>,
    except: Vec<String>,
}

impl RailsResourceActions {
    fn declares(&self, action: &str) -> bool {
        if let Some(only) = &self.only {
            return only.iter().any(|name| name == action);
        }
        !self.except.iter().any(|name| name == action)
    }
}

fn rails_resource_actions(rest: &str) -> RailsResourceActions {
    let mut actions = RailsResourceActions::default();
    for (option, target) in [("only:", true), ("except:", false)] {
        let Some((_, after)) = rest.split_once(option) else {
            continue;
        };
        let list = after.trim_start();
        let names: Vec<String> = if let Some(inner) = list.strip_prefix('[') {
            inner
                .split(']')
                .next()
                .unwrap_or_default()
                .split(',')
                .filter_map(ruby_symbol_name)
                .collect()
        } else {
            ruby_symbol_name(list).into_iter().collect()
        };
        if target {
            actions.only.get_or_insert_default().extend(names);
        } else {
            actions.except.extend(names);
        }
    }
    actions
}

/// The module a block states: `namespace :api` is `Api`, `scope module:
/// :v1` is `V1`, and everything else states none.
fn rails_block_module(line: &str) -> String {
    let trimmed = line.trim();
    let Some(rest) = trimmed.strip_prefix("scope ") else {
        return String::new();
    };
    rails_option_value(rest, "module")
        .map(|name| rails_module_name(&name))
        .unwrap_or_default()
}

/// What Rails calls the module a symbol names: `v1_alpha` is `V1Alpha`,
/// `activitypub` is `Activitypub` unless the project says otherwise.
fn rails_module_name(name: &str) -> String {
    name.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), characters.as_str()),
                None => String::new(),
            }
        })
        .collect()
}

/// The controller a route names, under the modules the open blocks state.
fn qualified_rails_controller(modules: &[String], controller: &str) -> Option<String> {
    let class = rails_controller_class(controller)?;
    let path: Vec<&str> = modules
        .iter()
        .map(String::as_str)
        .filter(|part| !part.is_empty())
        .collect();
    if path.is_empty() {
        return Some(class);
    }
    Some(format!("{}::{class}", path.join("::")))
}

/// The plural Rails names a controller with: `resource :setup` is served
/// by `SetupsController` and `resource :additional_footer_text` by
/// `AdditionalFooterTextsController`. Only the endings a route key can
/// carry are covered -- the inflector is a table nobody can hold.
fn rails_plural(name: &str) -> String {
    if name.ends_with('s')
        || name.ends_with('x')
        || name.ends_with('z')
        || name.ends_with("ch")
        || name.ends_with("sh")
    {
        return format!("{name}es");
    }
    if let Some(stem) = name.strip_suffix('y')
        && !stem.ends_with(['a', 'e', 'i', 'o', 'u'])
    {
        return format!("{stem}ies");
    }
    format!("{name}s")
}

/// The singular Rails writes for a resource name: `statuses` is
/// `status` and `policies` is `policy`. Only the endings that matter to
/// a route key are covered -- the inflector is a table nobody can carry.
fn rails_singular(name: &str) -> String {
    for ending in ["sses", "ses", "xes", "zes", "ches", "shes"] {
        if let Some(stem) = name.strip_suffix(ending) {
            return match ending {
                "sses" => format!("{stem}ss"),
                "ses" => format!("{stem}s"),
                _ => stem.to_string(),
            };
        }
    }
    if let Some(stem) = name.strip_suffix("ies") {
        return format!("{stem}y");
    }
    name.strip_suffix('s').unwrap_or(name).to_string()
}

/// The path the open blocks state, skipping the ones that add nothing:
/// every block pushes an entry so that every `end` pops one.
/// The stack entry a `collection do` block pushes. It is not a path
/// segment; `joined_route_prefix` reads it as "the routes inside are the
/// set's, not one member's".
const RAILS_COLLECTION_BLOCK: &str = "\u{1}collection";

/// The stack entry a `member do` block pushes. The routes inside serve
/// one member of the set, and Rails names its id `:id` -- `:user_id` is
/// what a resource nested inside one is given.
const RAILS_MEMBER_BLOCK: &str = "\u{1}member";

fn joined_route_prefix(prefixes: &[String]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for prefix in prefixes.iter().map(String::as_str) {
        // A `collection do` block holds routes for the set, not for one of
        // its members: `post :accept` there is
        // `/notifications/requests/accept`. The enclosing `resources` block
        // hands down its member path, which is right for a nested resource
        // and for `member do`, so a collection block takes the id back off.
        if prefix == RAILS_COLLECTION_BLOCK {
            if let Some(member) = parts.pop() {
                let head = member.split("/:").next().unwrap_or(&member).to_string();
                parts.push(head);
            }
            continue;
        }
        // `member do get :download end` under `resources :backups` is
        // `/backups/:id/download`: the enclosing block hands down the name
        // a resource nested inside it would use, and a member route of the
        // resource itself uses `:id`.
        if prefix == RAILS_MEMBER_BLOCK {
            if let Some(member) = parts.pop() {
                match member.split_once("/:") {
                    Some((head, _)) => parts.push(format!("{head}/:id")),
                    None => parts.push(member),
                }
            }
            continue;
        }
        if !prefix.is_empty() {
            parts.push(prefix.to_string());
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("/{}", parts.join("/"))
    }
}

/// Whether a line opens a block that an `end` will close.
fn opens_a_ruby_block(line: &str) -> bool {
    let trimmed = line.trim_end();
    trimmed.ends_with(" do")
        || trimmed.ends_with('|') && trimmed.contains(" do |")
        || [
            "if ", "unless ", "case ", "begin", "class ", "module ", "def ",
        ]
        .iter()
        .any(|opener| trimmed.starts_with(opener))
}

/// What a block adds to the path. `scope :v1_alpha do` and `scope path:
/// 'ap' do` prefix what they hold; `member do` and `collection do` do
/// not.
fn rails_block_prefix(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed == "collection do" {
        return RAILS_COLLECTION_BLOCK.to_string();
    }
    if trimmed == "member do" {
        return RAILS_MEMBER_BLOCK.to_string();
    }
    let Some(rest) = trimmed.strip_prefix("scope ") else {
        return String::new();
    };
    rails_option_value(rest, "path")
        .or_else(|| {
            // `scope :v1_alpha do` names the segment directly; `scope
            // module: :v1 do` names a module and no segment.
            let head = rest.split(',').next().unwrap_or_default().trim();
            head.strip_suffix(" do")
                .or(Some(head))
                .and_then(ruby_symbol_name)
        })
        .unwrap_or_default()
}

/// A route line read together with the options the blocks around it hand
/// down. The line's own options come first, and every reader here takes
/// the first spelling of a key, so what the line states still wins.
fn rails_line_with_inherited_options(rest: &str, inherited: &[Option<String>]) -> String {
    let mut line = rest.to_string();
    for options in inherited.iter().rev().flatten() {
        line.push_str(", ");
        line.push_str(options);
    }
    line
}

/// The value of a `key: 'value'` option on a route line.
fn rails_option_value(rest: &str, key: &str) -> Option<String> {
    let (_, after) = rest.split_once(&format!("{key}:"))?;
    let after = after.trim_start();
    first_quoted_value(after)
        .or_else(|| ruby_symbol_name(after))
        .filter(|value| !value.is_empty())
}

fn rails_resource_routes(
    prefix: &str,
    name: &str,
    controller: Option<String>,
    singular: bool,
    actions: &RailsResourceActions,
    line: u32,
) -> Vec<FrameworkRoute> {
    let base = format!("{prefix}/{name}");
    let member = if singular {
        base.clone()
    } else {
        format!("{base}/:id")
    };
    let mut declared = vec![
        ("GET", base.clone(), "index"),
        ("POST", base.clone(), "create"),
        ("GET", format!("{base}/new"), "new"),
        ("GET", member.clone(), "show"),
        ("GET", format!("{member}/edit"), "edit"),
        ("PATCH", member.clone(), "update"),
        ("PUT", member.clone(), "update"),
        ("DELETE", member, "destroy"),
    ];
    if singular {
        // A singular resource has no collection to list.
        declared.retain(|(_, _, action)| *action != "index");
    }
    declared.retain(|(_, _, action)| actions.declares(action));
    declared
        .into_iter()
        .map(|(method, path, action)| FrameworkRoute {
            framework: "rails".to_string(),
            method: method.to_string(),
            path,
            handler: Some(action.to_string()),
            handler_qualifier: controller.clone(),
            expanded: true,
            constrained: false,
            line,
        })
        .collect()
}

pub(crate) fn php_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    let mut routes = laravel_routes(source);
    let mut pending = Vec::new();

    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.starts_with("#[") && trimmed.contains("Route(") {
            if let Some(path) = first_quoted_value(trimmed) {
                pending.push(FrameworkRoute {
                    framework: "php-attribute".to_string(),
                    method: method_from_php_route(trimmed)
                        .unwrap_or("ROUTE")
                        .to_string(),
                    path,
                    handler: None,
                    handler_qualifier: None,
                    expanded: false,
                    constrained: false,
                    line: line_number,
                });
            }
            continue;
        }
        if let Some(function) = trimmed
            .strip_prefix("function ")
            .and_then(|rest| rest.split_once('(').map(|(name, _)| name.trim()))
            .filter(|name| !name.is_empty())
        {
            for mut route in pending.drain(..) {
                route.handler = Some(function.to_string());
                routes.push(route);
            }
        } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
            pending.clear();
        }
    }

    routes
}

/// Laravel states its routes in a file of `Route::` calls, and a group
/// gives its prefix to everything it holds: koel declares 147 of them and
/// the graph held none, because the PHP extractor knew only Symfony's
/// `#[Route]` attribute.
fn laravel_routes(source: &str) -> Vec<FrameworkRoute> {
    if !source.contains("Route::") {
        return Vec::new();
    }
    let mut routes = Vec::new();
    // The prefix a group opened, and the brace depth it holds until.
    let mut groups: Vec<(usize, String)> = Vec::new();
    // A chain states its prefix before it opens the group, and koel writes
    // the two on different lines.
    let mut pending_prefix: Option<String> = None;
    let mut depth = 0usize;
    let lines: Vec<&str> = source.lines().collect();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        while groups
            .last()
            .is_some_and(|(open_depth, _)| depth < *open_depth)
        {
            groups.pop();
        }
        let prefix = groups
            .iter()
            .map(|(_, prefix)| prefix.as_str())
            .filter(|prefix| !prefix.is_empty())
            .collect::<Vec<_>>()
            .join("/");

        if let Some(prefix) = laravel_group_prefix(trimmed) {
            pending_prefix = Some(prefix);
        }
        if let Some(verb) = laravel_route_verb(trimmed) {
            let arguments = trimmed
                .split_once(&format!("{verb}("))
                .map(|(_, rest)| rest)
                .unwrap_or_default();
            if let Some(path) = first_quoted_value(arguments) {
                let handler = laravel_route_handler(arguments);
                let joined = join_laravel_path(&prefix, &path);
                match verb {
                    // `->except('update', 'destroy')` limits what the
                    // expansion declares, and koel writes it on the line
                    // below the resource.
                    "apiResource" | "resource" => routes.extend(laravel_resource_routes(
                        &prefix,
                        &path,
                        handler.as_ref().map(|(_, owner)| owner.as_str()),
                        verb == "resource",
                        &laravel_chained_actions(&lines, index),
                        line_number,
                    )),
                    _ => routes.push(FrameworkRoute {
                        framework: "laravel".to_string(),
                        method: verb.to_ascii_uppercase(),
                        path: joined,
                        handler: handler.as_ref().map(|(handler, _)| handler.clone()),
                        handler_qualifier: handler.map(|(_, owner)| owner),
                        expanded: false,
                        constrained: false,
                        line: line_number,
                    }),
                }
            }
        }

        // `Route::prefix('api')->group(...)` and `->prefix('api')->group(`
        // both hand their prefix to the block that opens on the line.
        let opened = line.matches('{').count();
        let closed = line.matches('}').count();
        if opened > closed && trimmed.contains("->group(") {
            if let Some(prefix) = pending_prefix.take() {
                groups.push((depth + 1, prefix));
            } else {
                // A group with no prefix of its own still holds a depth, so
                // the prefixes above it are not popped by its braces.
                groups.push((depth + 1, String::new()));
            }
        }
        // A statement that ends without opening a group takes its prefix
        // with it.
        if trimmed.ends_with(';') {
            pending_prefix = None;
        }
        depth = depth + opened - closed.min(depth + opened);
    }
    routes
}

/// The verb a `Route::` call names, when the call declares a route.
fn laravel_route_verb(line: &str) -> Option<&'static str> {
    let rest = line.split_once("Route::")?.1;
    let verb = rest.split_once('(')?.0.trim();
    matches!(
        verb,
        "get"
            | "post"
            | "put"
            | "patch"
            | "delete"
            | "options"
            | "any"
            | "apiResource"
            | "resource"
    )
    .then(|| match verb {
        "get" => "get",
        "post" => "post",
        "put" => "put",
        "patch" => "patch",
        "delete" => "delete",
        "options" => "options",
        "any" => "any",
        "apiResource" => "apiResource",
        _ => "resource",
    })
}

/// The controller a route hands the request to, and the method it calls
/// on it: `[SongController::class, 'update']` is `update` on
/// `SongController`, and `SongController::class` on its own is the
/// invokable controller's `__invoke`.
fn laravel_route_handler(arguments: &str) -> Option<(String, String)> {
    let after_path = arguments.split_once(',')?.1.trim();
    if let Some(inner) = after_path.strip_prefix('[') {
        let controller = inner.split_once("::class")?.0.trim().rsplit('\\').next()?;
        let method = first_quoted_value(inner)?;
        return (!controller.is_empty() && !method.is_empty())
            .then(|| (method, controller.to_string()));
    }
    if let Some((controller, _)) = after_path.split_once("::class") {
        let controller = controller.trim().rsplit('\\').next()?;
        return (!controller.is_empty()).then(|| ("__invoke".to_string(), controller.to_string()));
    }
    // `'PostController@show'`, the older string form.
    let quoted = first_quoted_value(after_path)?;
    let (controller, method) = quoted.split_once('@')?;
    (!controller.is_empty() && !method.is_empty())
        .then(|| (method.to_string(), controller.to_string()))
}

/// The prefix a group states, from either `Route::prefix('api')` or a
/// `->prefix('api')` in the chain.
fn laravel_group_prefix(line: &str) -> Option<String> {
    let rest = line.split_once("prefix(")?.1;
    first_quoted_value(rest).filter(|prefix| !prefix.is_empty())
}

fn join_laravel_path(prefix: &str, path: &str) -> String {
    let path = path.trim_matches('/');
    let prefix = prefix.trim_matches('/');
    match (prefix.is_empty(), path.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{path}"),
        (false, true) => format!("/{prefix}"),
        (false, false) => format!("/{prefix}/{path}"),
    }
}

/// What `Route::apiResource('albums', AlbumController::class)` declares.
/// A nested resource is written with a dot -- `artists.albums` -- and the
/// parent's key sits between the two.
fn laravel_resource_routes(
    prefix: &str,
    name: &str,
    controller: Option<&str>,
    with_forms: bool,
    limits: &LaravelResourceLimits,
    line: u32,
) -> Vec<FrameworkRoute> {
    // The resource's own name states the nesting; the prefix the group
    // gave it is joined afterwards, or the parent key reads
    // `{/api/album}`.
    let segments: Vec<&str> = name.split('.').collect();
    let mut base = String::new();
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            let parent = segments[index - 1].trim_end_matches('s');
            base.push_str(&format!("/{{{parent}}}"));
        }
        if index > 0 {
            base.push('/');
        }
        base.push_str(segment);
    }
    let key = segments
        .last()
        .copied()
        .unwrap_or("id")
        .trim_end_matches('s');
    let member = format!("{base}/{{{key}}}");
    let mut declared = vec![
        ("GET", base.clone(), "index"),
        ("POST", base.clone(), "store"),
        ("GET", member.clone(), "show"),
        ("PUT", member.clone(), "update"),
        ("DELETE", member.clone(), "destroy"),
    ];
    if with_forms {
        declared.push(("GET", format!("{base}/create"), "create"));
        declared.push(("GET", format!("{member}/edit"), "edit"));
    }
    declared
        .into_iter()
        .filter(|(_, _, action)| limits.declares(action))
        .map(|(method, path, action)| FrameworkRoute {
            framework: "laravel".to_string(),
            method: method.to_string(),
            path: join_laravel_path(prefix, &path),
            handler: Some(action.to_string()),
            handler_qualifier: controller.map(str::to_string),
            expanded: true,
            constrained: false,
            line,
        })
        .collect()
}

/// What `->only(..)` and `->except(..)` say about a resource, read from
/// the chain that follows it -- koel writes `->except('update',
/// 'destroy')` on the line below the resource itself.
#[derive(Default)]
struct LaravelResourceLimits {
    only: Vec<String>,
    except: Vec<String>,
}

impl LaravelResourceLimits {
    fn declares(&self, action: &str) -> bool {
        if !self.only.is_empty() {
            return self.only.iter().any(|name| name == action);
        }
        !self.except.iter().any(|name| name == action)
    }
}

fn laravel_chained_actions(lines: &[&str], index: usize) -> LaravelResourceLimits {
    let mut limits = LaravelResourceLimits::default();
    for line in lines.iter().skip(index).take(4) {
        let trimmed = line.trim();
        for (opener, target) in [("->only(", true), ("->except(", false)] {
            if let Some(rest) = trimmed.split_once(opener).map(|(_, rest)| rest) {
                let list = rest.split(')').next().unwrap_or_default();
                let names: Vec<String> = list.split(',').filter_map(first_quoted_value).collect();
                if target {
                    limits.only.extend(names);
                } else {
                    limits.except.extend(names);
                }
            }
        }
        if trimmed.ends_with(';') {
            break;
        }
    }
    limits
}

/// ASP.NET writes a route as an attribute above the action that serves it,
/// with the controller stating a template of its own —
/// `[Route("[controller]/[action]")]` — and a minimal API writes
/// `app.MapGet("/health", ..)`. eShopOnWeb declares 19 the first way.
pub(crate) fn csharp_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    let mut routes = Vec::new();
    let mut pending: Vec<FrameworkRoute> = Vec::new();
    let mut class_prefix = String::new();
    let mut prefix_for_next_class: Option<String> = None;
    let mut controller: Option<String> = None;

    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*") {
            continue;
        }
        // `app.MapGet("/health", () => ..)` is the whole declaration. The
        // verb is part of the method name, so the `.get(` a route call
        // usually ends in never appears and eShopOnWeb's nineteen minimal
        // API endpoints were missing from a graph that found its
        // attribute routes.
        if let Some(route) = csharp_minimal_api_route(line, line_number) {
            routes.push(route);
            continue;
        }
        if trimmed.starts_with('[') {
            if let Some((method, has_own_path)) = csharp_route_attribute(trimmed) {
                let path = first_quoted_value(trimmed).unwrap_or_default();
                // An attribute outside a member states where the
                // controller's actions live.
                if !has_own_path && !path.is_empty() {
                    prefix_for_next_class = Some(path.clone());
                    continue;
                }
                pending.push(FrameworkRoute {
                    framework: "asp.net".to_string(),
                    method: method.to_string(),
                    path,
                    handler: None,
                    handler_qualifier: None,
                    expanded: false,
                    constrained: false,
                    line: line_number,
                });
            }
            continue;
        }
        if trimmed.contains("class ") {
            class_prefix = prefix_for_next_class.take().unwrap_or_default();
            controller = csharp_controller_name(trimmed);
            pending.clear();
            continue;
        }
        if let Some(name) = jvm_method_name(trimmed) {
            for mut route in pending.drain(..) {
                let path = join_jvm_route_path(&class_prefix, &route.path);
                route.path = expand_csharp_route_tokens(&path, controller.as_deref(), &name);
                route.handler = Some(name.clone());
                routes.push(route);
            }
            continue;
        }
        if !trimmed.is_empty() && !trimmed.starts_with('}') && !trimmed.starts_with('{') {
            pending.clear();
        }
    }

    routes
}

/// The controller a class declares, without the suffix ASP.NET strips:
/// `OrderController` serves `/Order`.
fn csharp_controller_name(line: &str) -> Option<String> {
    let rest = line.split("class ").nth(1)?;
    let name = rest
        .split([' ', ':', '{', '<', '('])
        .find(|part| !part.is_empty())?;
    let name = name.strip_suffix("Controller").unwrap_or(name);
    (!name.is_empty()).then(|| name.to_string())
}

/// What the framework fills into a route template: `[controller]` is the
/// class's own name and `[action]` the method's, so eShopOnWeb's 25 actions
/// serve 25 URLs rather than one written 25 times.
fn expand_csharp_route_tokens(path: &str, controller: Option<&str>, action: &str) -> String {
    let mut expanded = path.to_string();
    if let Some(controller) = controller {
        expanded = expanded.replace("[controller]", controller);
    }
    expanded.replace("[action]", action)
}

/// The method a C# route attribute states, and whether the attribute
/// belongs to an action rather than to the controller: `[HttpGet]` and
/// `[HttpGet("{id}")]` are an action's, `[Route("[controller]")]` above a
/// class is the controller's.
fn csharp_route_attribute(line: &str) -> Option<(&'static str, bool)> {
    let name = line.trim_start_matches('[');
    for (attribute, method) in [
        ("HttpGet", "GET"),
        ("HttpPost", "POST"),
        ("HttpPut", "PUT"),
        ("HttpDelete", "DELETE"),
        ("HttpPatch", "PATCH"),
        ("HttpHead", "HEAD"),
    ] {
        if name.starts_with(attribute) {
            return Some((method, true));
        }
    }
    name.starts_with("Route(").then_some(("ROUTE", false))
}

/// Spring and JAX-RS write a route as an annotation above the method that
/// serves it, with the class stating a prefix of its own:
/// `@RequestMapping("/owners")` on the class and `@GetMapping("/{id}")` on
/// the method. spring-petclinic declares 21 that way.
pub(crate) fn jvm_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    if !(source.contains("Mapping") || source.contains("@Path")) {
        return Vec::new();
    }
    let mut routes = Vec::new();
    let mut pending: Vec<FrameworkRoute> = Vec::new();
    let mut class_prefix = String::new();
    let mut prefix_for_next_class: Option<String> = None;

    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*") {
            continue;
        }
        if trimmed.starts_with('@') {
            if let Some((method, framework)) = jvm_route_annotation(trimmed) {
                let path = first_quoted_value(trimmed).unwrap_or_default();
                // An annotation on the class states where its methods live.
                if trimmed.starts_with("@RequestMapping")
                    && !line.starts_with('\t')
                    && !path.is_empty()
                {
                    prefix_for_next_class = Some(path.clone());
                }
                pending.push(FrameworkRoute {
                    framework: framework.to_string(),
                    method: method.to_string(),
                    path,
                    handler: None,
                    handler_qualifier: None,
                    expanded: false,
                    constrained: false,
                    line: line_number,
                });
            }
            continue;
        }
        if trimmed.contains("class ") && trimmed.contains('{') {
            class_prefix = prefix_for_next_class.take().unwrap_or_default();
            // The annotation that stated the prefix is the class's own, not
            // a route of its own.
            pending.retain(|route| route.path != class_prefix);
            continue;
        }
        if let Some(name) = jvm_method_name(trimmed) {
            for mut route in pending.drain(..) {
                route.path = join_jvm_route_path(&class_prefix, &route.path);
                route.handler = Some(name.clone());
                routes.push(route);
            }
            continue;
        }
        if !trimmed.is_empty() && !trimmed.starts_with('}') {
            pending.clear();
        }
    }

    routes
}

/// The method and framework a JVM route annotation states.
fn jvm_route_annotation(line: &str) -> Option<(&'static str, &'static str)> {
    let name = line.trim_start_matches('@');
    for (annotation, method) in [
        ("GetMapping", "GET"),
        ("PostMapping", "POST"),
        ("PutMapping", "PUT"),
        ("DeleteMapping", "DELETE"),
        ("PatchMapping", "PATCH"),
    ] {
        if name.starts_with(annotation) {
            return Some((method, "spring"));
        }
    }
    if name.starts_with("RequestMapping") {
        let method = ["GET", "POST", "PUT", "DELETE", "PATCH"]
            .into_iter()
            .find(|method| line.contains(&format!("RequestMethod.{method}")))
            .unwrap_or("ROUTE");
        return Some((method, "spring"));
    }
    // Retrofit writes `@Path("id")` on a parameter to name a URL segment,
    // and JAX-RS writes `@Path("/users")` on the resource: a path starts
    // with a slash, and a parameter name does not.
    if name.starts_with("Path(")
        && first_quoted_value(line).is_some_and(|value| value.starts_with('/'))
    {
        return Some(("ROUTE", "jax-rs"));
    }
    None
}

/// The name of the method a JVM route annotation sits above.
fn jvm_method_name(line: &str) -> Option<String> {
    let (head, _) = line.split_once('(')?;
    let name = head.split_whitespace().next_back()?;
    let name = name.trim_start_matches('*');
    (!name.is_empty()
        && name
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_')
        && name.chars().next().is_some_and(char::is_alphabetic)
        && !matches!(
            name,
            "if" | "for" | "while" | "switch" | "catch" | "return" | "new" | "class"
        ))
    .then(|| name.to_string())
}

/// The path a route serves: the class's prefix and the method's own.
fn join_jvm_route_path(prefix: &str, path: &str) -> String {
    let prefix = prefix.trim_end_matches('/');
    let path = path.trim();
    let joined = match (prefix.is_empty(), path.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => path.to_string(),
        (false, true) => prefix.to_string(),
        (false, false) => format!("{prefix}/{}", path.trim_start_matches('/')),
    };
    if joined.starts_with('/') {
        joined
    } else {
        format!("/{joined}")
    }
}

/// `app.MapGet("api/catalog-items/{id}", ..)`: the minimal API way to
/// declare a route, where the verb is part of the method name. The path
/// is written without a leading slash as often as with one, and the URL
/// is the same either way.
fn csharp_minimal_api_route(line: &str, line_number: u32) -> Option<FrameworkRoute> {
    if !line.contains(".Map") {
        return None;
    }
    let method = route_methods().iter().find(|method| {
        let mut needle = String::from(".Map");
        needle.push_str(&method[..1]);
        needle.push_str(&method[1..].to_ascii_lowercase());
        needle.push('(');
        line.contains(&needle)
    })?;
    let path = first_quoted_value(line)?;
    let path = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };
    Some(FrameworkRoute {
        framework: "asp.net".to_string(),
        method: (*method).to_string(),
        path,
        handler: handler_after_first_comma(line),
        handler_qualifier: None,
        expanded: false,
        constrained: false,
        line: line_number,
    })
}

pub(crate) fn route_from_call_line(
    line: &str,
    line_number: u32,
    framework: &str,
    allowed_receivers: &[&str],
) -> Option<FrameworkRoute> {
    // Every line of every file reaches this, so the cheap facts come
    // first: a route calls something and names its path with a string
    // literal, and a line with neither cannot be one.
    if !line.contains('(') || !line.contains(['"', '\'', '`']) {
        return None;
    }
    let lower = line.to_ascii_lowercase();
    let method = route_method_needles()
        .iter()
        .find(|(_, dotted, arrowed)| {
            route_receiver_matches(&lower, dotted, arrowed, allowed_receivers)
        })
        .map(|(method, _, _)| *method)?;
    let path = first_quoted_value(line)?;
    let handler = handler_after_first_comma(line);
    Some(FrameworkRoute {
        framework: framework.to_string(),
        method: method.to_string(),
        path,
        handler,
        handler_qualifier: None,
        expanded: false,
        constrained: false,
        line: line_number,
    })
}

/// The two shapes a route call takes, lowercased once rather than built
/// afresh for every method on every line of every file.
fn route_method_needles() -> &'static [(&'static str, String, String)] {
    static NEEDLES: std::sync::OnceLock<Vec<(&'static str, String, String)>> =
        std::sync::OnceLock::new();
    NEEDLES.get_or_init(|| {
        route_methods()
            .iter()
            .map(|method| {
                let lowered = method.to_ascii_lowercase();
                (*method, format!(".{lowered}("), format!("->{lowered}("))
            })
            .collect()
    })
}

pub(crate) fn route_receiver_matches(
    line: &str,
    dotted: &str,
    arrowed: &str,
    allowed_receivers: &[&str],
) -> bool {
    let Some(method_index) = line.find(dotted).or_else(|| line.find(arrowed)) else {
        return false;
    };
    let receiver = line[..method_index]
        .rsplit(|character: char| {
            !(character.is_ascii_alphanumeric() || matches!(character, '_' | '$'))
        })
        .next()
        .unwrap_or("")
        .trim_start_matches('$');
    allowed_receivers
        .iter()
        .any(|allowed| receiver.eq_ignore_ascii_case(allowed))
}

pub(crate) fn handler_from_rust_route(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    for method in route_methods() {
        let needle = format!("{}(", method.to_ascii_lowercase());
        if let Some(start) = find_unquoted(&lower, &needle) {
            let rest = &line[start + needle.len()..];
            let handler = rest
                .split([',', ')'])
                .next()
                .map(|value| value.trim().trim_start_matches("move ").trim())
                .filter(|value| !value.is_empty())?;
            return Some(handler.to_string());
        }
    }
    None
}

pub(crate) fn rust_route_call_window(lines: &[&str], start_index: usize) -> String {
    let mut call = String::new();
    for line in lines.iter().skip(start_index).take(12) {
        if !call.is_empty() {
            call.push(' ');
        }
        call.push_str(line.trim());
        if rust_route_call_closed(&call) {
            break;
        }
    }
    call
}

pub(crate) fn rust_route_call_closed(value: &str) -> bool {
    let Some(route_index) = find_unquoted(value, ".route(") else {
        return false;
    };
    let mut quote = None;
    let mut escaped = false;
    let mut depth = 0_i32;
    let mut started = false;

    for (_, character) in value[route_index..].char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
                continue;
            }
            if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'' | '`') {
            quote = Some(character);
            continue;
        }
        if character == '(' {
            depth += 1;
            started = true;
        } else if character == ')' && started {
            depth -= 1;
            if depth == 0 {
                return true;
            }
        }
    }

    false
}

pub(crate) fn handler_after_first_comma(line: &str) -> Option<String> {
    let handler = line
        .split_once(',')
        .map(|(_, rest)| rest.trim())
        .and_then(|rest| rest.split([',', ')']).next())
        .map(|value| {
            value
                .trim()
                .trim_start_matches('&')
                .trim_start_matches("::")
                .trim_matches(['"', '\'', '`'])
                .to_string()
        })
        .filter(|value| !value.is_empty())?;
    names_a_handler(&handler).then_some(handler)
}

/// Whether what follows a route path is the name of a handler, or the
/// start of one written in place. `func(w http.ResponseWriter`, `(req`,
/// `|request|` and `multer(` open a function literal or a call that builds
/// one: there is no name in them to look up, and the handler is already
/// there at the route for anyone reading it.
fn names_a_handler(handler: &str) -> bool {
    !handler.starts_with(|character: char| character.is_ascii_digit())
        && handler.chars().all(|character| {
            character.is_alphanumeric()
                || matches!(character, '_' | '.' | ':' | '$' | '#' | '@' | '\\')
        })
}

pub(crate) fn method_from_php_route(line: &str) -> Option<&'static str> {
    let upper = line.to_ascii_uppercase();
    route_methods()
        .iter()
        .find(|method| {
            upper.contains(&format!("\"{method}\"")) || upper.contains(&format!("'{method}'"))
        })
        .copied()
}

pub(crate) fn first_quoted_value(value: &str) -> Option<String> {
    let quote_index = value.find(['"', '\'', '`'])?;
    let quote = value[quote_index..].chars().next()?;
    let rest = &value[quote_index + quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

pub(crate) fn first_quoted_value_after(value: &str, needle: &str) -> Option<String> {
    let lower_value = value.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    let start = lower_value.find(&lower_needle)?;
    first_quoted_value(&value[start + needle.len()..])
}

pub(crate) fn find_unquoted(value: &str, needle: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in value.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
                continue;
            }
            if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'' | '`') {
            quote = Some(character);
            continue;
        }
        if value[index..].starts_with(needle) {
            return Some(index);
        }
    }

    None
}

pub(crate) fn route_methods() -> &'static [&'static str] {
    &["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"]
}

/// A route a JavaScript framework declares by where the file sits rather
/// than by a call in it: Next.js, Nuxt and SvelteKit all do this, and a
/// project written that way had no entrypoints at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileRoute {
    pub(crate) framework: &'static str,
    /// The npm package whose presence says the project really is written
    /// this way. `app/` is a PHP directory as often as a Next.js one.
    pub(crate) package: &'static str,
    pub(crate) path: String,
    pub(crate) shape: FileRouteShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileRouteShape {
    /// A page: the framework serves it on GET.
    Page,
    /// A handler module: each exported HTTP verb is a route of its own.
    Handler,
    /// A handler that serves every method, which is what Next.js's pages
    /// API routes do.
    AnyMethod,
    /// A Razor Page: the `.cshtml` file states the URL and the `.cshtml.cs`
    /// beside it holds the handlers, one `OnGet`/`OnPost` per method.
    PageModel,
    /// A file the framework runs rather than serves: a layout, an error
    /// boundary, middleware. It has no URL of its own, and everything it
    /// renders runs on every request that passes through it.
    Entry,
}

/// The route a file's own path declares, when its project is written that
/// way. The path is the evidence: `app/api/users/route.ts` is
/// `/api/users`, `app/blog/[slug]/page.tsx` is `/blog/:slug`, and a
/// `(marketing)` segment groups files without naming a URL.
/// A Razor Page says so itself: `@page` at the top of a `.cshtml` file
/// means ASP.NET serves it at the path the file sits on under the
/// `Pages/` directory holding it, and an area names itself first --
/// `Areas/Identity/Pages/Account/Login.cshtml` is `/Identity/Account/Login`.
/// A Blazor component writes the URL out: `@page "/admin"`. Neither needs
/// a manifest to confirm it, because the file states it.
/// The method a Razor Page handler serves: `OnGet`, `OnPostAsync`,
/// `OnGetDeleteAsync` -- the verb follows `On`, and anything after it
/// names the handler rather than the method.
pub(crate) fn razor_handler_method(name: &str) -> Option<&'static str> {
    let rest = name.strip_prefix("On")?;
    route_methods()
        .iter()
        .find(|method| {
            let mut verb = String::from(&method[..1]);
            verb.push_str(&method[1..].to_ascii_lowercase());
            rest.strip_prefix(&verb).is_some_and(|tail| {
                tail.is_empty() || tail.starts_with(|character: char| character.is_uppercase())
            })
        })
        .copied()
}

pub(crate) fn razor_page_route(label: &str, source: &str) -> Option<FileRoute> {
    let normalized = label.replace('\\', "/");
    let blazor = normalized.ends_with(".razor");
    if !blazor && !normalized.ends_with(".cshtml") {
        return None;
    }
    // The directive opens the file, ahead of anything but a byte-order
    // mark, `@using` lines and blanks.
    let template = source
        .lines()
        .take(20)
        .map(|line| line.trim_start_matches('\u{feff}').trim())
        .find(|line| line == &"@page" || line.starts_with("@page "))
        .map(|line| first_quoted_value(line).unwrap_or_default())?;
    if blazor {
        // A Blazor page with no path states nothing to serve.
        let path = template.trim();
        if !path.starts_with('/') {
            return None;
        }
        return Some(FileRoute {
            framework: "asp.net",
            package: "",
            path: path.to_string(),
            shape: FileRouteShape::Page,
        });
    }
    // `@page "/custom"` replaces the conventional path; anything else --
    // `@page "{handler?}"`, `@page "{id:int}"` -- extends it.
    if template.starts_with('/') {
        return Some(FileRoute {
            framework: "asp.net",
            package: "",
            path: template,
            shape: FileRouteShape::PageModel,
        });
    }
    let mut path = razor_conventional_path(&normalized)?;
    if !template.is_empty() {
        if !path.ends_with('/') {
            path.push('/');
        }
        path.push_str(&template);
    }
    Some(FileRoute {
        framework: "asp.net",
        package: "",
        path,
        shape: FileRouteShape::PageModel,
    })
}

/// The URL a Razor Page sits on: what follows the `Pages/` directory,
/// with the area that holds it in front and `Index` standing for the
/// directory itself.
fn razor_conventional_path(normalized: &str) -> Option<String> {
    let stem = normalized.strip_suffix(".cshtml")?;
    let (before, rest) = stem.split_once("/Pages/").or_else(|| {
        stem.strip_prefix("Pages/")
            .map(|rest| ("", rest))
            .or_else(|| stem.strip_prefix("Pages").map(|rest| ("", rest)))
    })?;
    let area = before
        .rsplit_once("/Areas/")
        .map(|(_, area)| area)
        .or_else(|| before.strip_prefix("Areas/"))
        .filter(|area| !area.is_empty() && !area.contains('/'));
    // A page named `Index` is the directory it sits in.
    let rest = rest
        .strip_suffix("Index")
        .map_or(rest, |head| head.strip_suffix('/').unwrap_or(head));
    let mut path = String::new();
    if let Some(area) = area {
        path.push('/');
        path.push_str(area);
    }
    if !rest.is_empty() {
        path.push('/');
        path.push_str(rest.trim_start_matches('/'));
    }
    if path.is_empty() {
        path.push('/');
    }
    Some(path)
}

pub(crate) fn file_based_route(label: &str) -> Option<FileRoute> {
    let normalized = label.replace('\\', "/");
    let mut segments: Vec<&str> = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    let file = segments.pop()?;
    let (stem, extension) = file.rsplit_once('.')?;
    // A `src/` wrapper is the same layout one directory down.
    if segments.first() == Some(&"src") {
        segments.remove(0);
    }
    let root = *segments.first()?;

    let js_module = matches!(extension, "ts" | "tsx" | "js" | "jsx" | "mts" | "mjs");
    match root {
        // What Next.js runs around a page: a layout wraps every route
        // beneath it, an error boundary catches what they throw, and
        // `middleware` runs on every request. None has a URL of its own,
        // and without them the components a layout renders -- eleven in
        // taxonomy -- are reached by nothing.
        "app"
            if js_module
                && matches!(
                    stem,
                    "layout"
                        | "template"
                        | "error"
                        | "global-error"
                        | "loading"
                        | "not-found"
                        | "default"
                ) =>
        {
            Some(FileRoute {
                framework: "next",
                package: "next",
                path: url_path_from_segments(&segments[1..])?,
                shape: FileRouteShape::Entry,
            })
        }
        // Next.js's app router: a directory is a URL segment, `route` is a
        // handler module and `page` is a page.
        "app" if js_module && matches!(stem, "route" | "page") => {
            let path = url_path_from_segments(&segments[1..])?;
            Some(FileRoute {
                framework: "next",
                package: "next",
                path,
                shape: if stem == "route" {
                    FileRouteShape::Handler
                } else {
                    FileRouteShape::Page
                },
            })
        }
        // Next.js's pages router, and Nuxt's, which writes `.vue` pages.
        "pages" if (js_module || extension == "vue") && !stem.starts_with('_') => {
            let mut parts: Vec<&str> = segments[1..].to_vec();
            // `pages/blog/index.tsx` serves `/blog`.
            if stem != "index" {
                parts.push(stem);
            }
            let path = url_path_from_segments(&parts)?;
            let api = segments.get(1) == Some(&"api");
            Some(FileRoute {
                framework: if extension == "vue" { "nuxt" } else { "next" },
                package: if extension == "vue" { "nuxt" } else { "next" },
                path,
                shape: if api {
                    FileRouteShape::AnyMethod
                } else {
                    FileRouteShape::Page
                },
            })
        }
        // SvelteKit runs a layout and an error page around its routes the
        // same way.
        "routes"
            if matches!(file, "+layout.svelte" | "+error.svelte")
                || (js_module && matches!(stem, "+layout" | "+layout.server" | "+page.server")) =>
        {
            Some(FileRoute {
                framework: "sveltekit",
                package: "@sveltejs/kit",
                path: url_path_from_segments(&segments[1..])?,
                shape: FileRouteShape::Entry,
            })
        }
        // SvelteKit: `+page.svelte` is a page and `+server.ts` a handler.
        "routes" if matches!(file, "+page.svelte") || (js_module && stem == "+server") => {
            let path = url_path_from_segments(&segments[1..])?;
            Some(FileRoute {
                framework: "sveltekit",
                package: "@sveltejs/kit",
                path,
                shape: if file == "+page.svelte" {
                    FileRouteShape::Page
                } else {
                    FileRouteShape::Handler
                },
            })
        }
        _ => None,
    }
}

/// The URL a directory path names. A `(group)` segment organises files
/// without naming a URL, a `@slot` segment is a parallel route rather than
/// a page, `[slug]` is a parameter and `[...rest]` catches what is left.
fn url_path_from_segments(segments: &[&str]) -> Option<String> {
    let mut parts = Vec::new();
    for segment in segments {
        // A private folder and a parallel route are not URL segments.
        if segment.starts_with('_') || segment.starts_with('@') {
            return None;
        }
        if segment.starts_with('(') && segment.ends_with(')') {
            continue;
        }
        let part = if let Some(inner) = segment
            .strip_prefix("[[...")
            .and_then(|rest| rest.strip_suffix("]]"))
        {
            format!("*{inner}")
        } else if let Some(inner) = segment
            .strip_prefix("[...")
            .and_then(|rest| rest.strip_suffix(']'))
        {
            format!("*{inner}")
        } else if let Some(inner) = segment
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            // SvelteKit writes `[slug=integer]` when it matches a pattern.
            let inner = inner.split('=').next().unwrap_or(inner);
            format!(":{inner}")
        } else {
            (*segment).to_string()
        };
        parts.push(part);
    }
    Some(if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    })
}

/// The HTTP methods a handler module exports. Next.js and SvelteKit both
/// name the function after the method it serves.
pub(crate) fn file_route_method(name: &str) -> Option<&'static str> {
    match name {
        "GET" => Some("GET"),
        "POST" => Some("POST"),
        "PUT" => Some("PUT"),
        "PATCH" => Some("PATCH"),
        "DELETE" => Some("DELETE"),
        "HEAD" => Some("HEAD"),
        "OPTIONS" => Some("OPTIONS"),
        _ => None,
    }
}

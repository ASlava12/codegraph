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
            handler: django_route_handler(&handler_source),
            line: index as u32 + 1,
        });
    }
    routes
}

/// The view a URLconf entry points at: `self.detail_view.as_view()` is
/// `detail_view`, `views.IndexView.as_view()` is `IndexView`, and
/// `include("oscar.apps.basket.urls")` names another URLconf rather than a
/// view.
fn django_route_handler(rest: &str) -> Option<String> {
    let after_path = rest.split_once(',')?.1;
    let candidate = after_path.split(',').next()?.trim();
    // `self.detail_view.as_view()` names an attribute of the app config,
    // whose value is assigned somewhere else entirely: django-oscar writes
    // 124 of them, and claiming a function called `detail_view` is a guess
    // the syntax cannot make good on.
    if candidate.is_empty() || candidate.starts_with("include(") || candidate.starts_with("self.") {
        return None;
    }
    let name = candidate
        .trim_end_matches("()")
        .trim_end_matches(".as_view")
        .rsplit('.')
        .next()?
        .trim();
    (!name.is_empty()
        && name
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_'))
    .then(|| name.to_string())
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
pub(crate) fn ruby_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    const METHODS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options"];
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            let (method, rest) = METHODS
                .iter()
                .find_map(|method| trimmed.strip_prefix(method).map(|rest| (*method, rest)))?;
            if !rest.starts_with(char::is_whitespace) {
                return None;
            }
            let path = first_quoted_value(rest)?;
            if !path.starts_with('/') {
                return None;
            }
            Some(FrameworkRoute {
                framework: "sinatra".to_string(),
                method: method.to_ascii_uppercase(),
                path,
                handler: None,
                line: index as u32 + 1,
            })
        })
        .collect()
}

pub(crate) fn php_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    let mut routes = Vec::new();
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
        // `app.MapGet("/health", () => ..)` is the whole declaration.
        if let Some(route) = route_from_call_line(
            line,
            line_number,
            "asp.net",
            &["app", "endpoints", "builder"],
        ) && line.contains(".Map")
        {
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

//! Build/CI/container/Kubernetes runtime surfaces: Makefile, Dockerfile,
//! Docker Compose, GitHub Actions, GitLab CI, and Kubernetes manifests
//! with their line-oriented parsers.

use std::collections::BTreeMap;
use std::path::Path;

use codegraph_core::{Confidence, EdgeKind, NodeId, NodeKind};

#[allow(unused_imports)]
use crate::*;

pub(crate) fn index_makefile_entrypoints(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_makefile_path(path) {
        return;
    }

    for target in makefile_targets(source) {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "makefile_target".to_string());
        metadata.insert("entrypoint_kind".to_string(), "make_target".to_string());
        metadata.insert("ecosystem".to_string(), "make".to_string());
        metadata.insert("source".to_string(), "makefile".to_string());
        metadata.insert("target".to_string(), target.name.clone());
        metadata.insert("line".to_string(), target.line.to_string());
        if let Some(command) = target.command.as_deref() {
            metadata.insert("command".to_string(), command.to_string());
            if let Some(command_path) = normalized_command_path_candidate(label, command) {
                metadata.insert("command_path".to_string(), command_path);
            }
        }

        let entrypoint_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("make target:{}", target.name),
            Some(line_span(label, source, target.line)),
            metadata,
        );
        add_edge_once(
            context,
            file_id,
            entrypoint_id,
            EdgeKind::Contains,
            Confidence::Exact,
        );
        let root_id = context.graph.root;
        add_edge_once(
            context,
            root_id,
            entrypoint_id,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        add_entrypoint_reference(
            context,
            entrypoint_id,
            file_id,
            "entrypoint_file",
            "makefile_target",
            Confidence::Exact,
            None,
        );

        if let Some(command) = target.command {
            context
                .pending_entrypoint_targets
                .push(PendingEntrypointTarget {
                    entrypoint: entrypoint_id,
                    manifest_label: label.to_string(),
                    target: command,
                    ecosystem: "make".to_string(),
                    entrypoint_kind: "make_target".to_string(),
                });
        }
    }
}

pub(crate) fn index_dockerfile_entrypoints(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_dockerfile_path(path) {
        return;
    }

    for entrypoint in dockerfile_entrypoints(source) {
        let instruction = entrypoint.instruction.to_ascii_lowercase();
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "dockerfile_entrypoint".to_string());
        metadata.insert("entrypoint_kind".to_string(), instruction.clone());
        metadata.insert("ecosystem".to_string(), "docker".to_string());
        metadata.insert("source".to_string(), "dockerfile".to_string());
        metadata.insert("instruction".to_string(), entrypoint.instruction.clone());
        metadata.insert("command".to_string(), entrypoint.command.clone());
        metadata.insert("line".to_string(), entrypoint.line.to_string());
        if let Some(command_path) = normalized_command_path_candidate(label, &entrypoint.command) {
            metadata.insert("command_path".to_string(), command_path);
        }

        let entrypoint_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("docker {instruction}:{}", entrypoint.command),
            Some(line_span(label, source, entrypoint.line)),
            metadata,
        );
        add_edge_once(
            context,
            file_id,
            entrypoint_id,
            EdgeKind::Contains,
            Confidence::Exact,
        );
        let root_id = context.graph.root;
        add_edge_once(
            context,
            root_id,
            entrypoint_id,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        add_entrypoint_reference(
            context,
            entrypoint_id,
            file_id,
            "entrypoint_file",
            "dockerfile_instruction",
            Confidence::Exact,
            None,
        );
        context
            .pending_entrypoint_targets
            .push(PendingEntrypointTarget {
                entrypoint: entrypoint_id,
                manifest_label: label.to_string(),
                target: entrypoint.command,
                ecosystem: "docker".to_string(),
                entrypoint_kind: instruction,
            });
    }
}

pub(crate) fn index_compose_entrypoints(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_compose_file_path(path) {
        return;
    }
    let services = compose_services(source);
    if services.is_empty() {
        return;
    }

    let mut service_nodes = BTreeMap::new();
    for service in &services {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "compose_service".to_string());
        metadata.insert("entrypoint_kind".to_string(), "service".to_string());
        metadata.insert("ecosystem".to_string(), "docker-compose".to_string());
        metadata.insert("source".to_string(), "compose".to_string());
        metadata.insert("service".to_string(), service.name.clone());
        metadata.insert("line".to_string(), service.line.to_string());
        if let Some(command) = service.command.as_deref() {
            metadata.insert("command".to_string(), command.to_string());
            if let Some(kind) = service.command_kind.as_deref() {
                metadata.insert("command_kind".to_string(), kind.to_string());
            }
            if let Some(command_path) = normalized_command_path_candidate(label, command) {
                metadata.insert("command_path".to_string(), command_path);
            }
        }
        if let Some(context_path) = service.build_context.as_deref() {
            metadata.insert("build_context".to_string(), context_path.to_string());
        }
        if let Some(dockerfile) = compose_service_dockerfile_path(label, service) {
            metadata.insert("dockerfile".to_string(), dockerfile);
        }
        if !service.environment.is_empty() {
            metadata.insert(
                "environment_count".to_string(),
                service.environment.len().to_string(),
            );
        }
        if !service.env_files.is_empty() {
            metadata.insert(
                "env_file_count".to_string(),
                service.env_files.len().to_string(),
            );
        }
        if !service.ports.is_empty() {
            metadata.insert("port_count".to_string(), service.ports.len().to_string());
        }
        if !service.volumes.is_empty() {
            metadata.insert(
                "volume_count".to_string(),
                service.volumes.len().to_string(),
            );
        }

        let entrypoint_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("compose service:{}", service.name),
            Some(line_span(label, source, service.line)),
            metadata,
        );
        service_nodes.insert(service.name.clone(), entrypoint_id);
        add_edge_once(
            context,
            file_id,
            entrypoint_id,
            EdgeKind::Contains,
            Confidence::Exact,
        );
        let root_id = context.graph.root;
        add_edge_once(
            context,
            root_id,
            entrypoint_id,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        add_entrypoint_reference(
            context,
            entrypoint_id,
            file_id,
            "entrypoint_file",
            "compose_service",
            Confidence::Exact,
            None,
        );
        if let Some(command) = service.command.clone() {
            context
                .pending_entrypoint_targets
                .push(PendingEntrypointTarget {
                    entrypoint: entrypoint_id,
                    manifest_label: label.to_string(),
                    target: command,
                    ecosystem: "compose".to_string(),
                    entrypoint_kind: "service".to_string(),
                });
        }
        if let Some(dockerfile) = compose_service_dockerfile_target(service) {
            context
                .pending_entrypoint_targets
                .push(PendingEntrypointTarget {
                    entrypoint: entrypoint_id,
                    manifest_label: label.to_string(),
                    target: dockerfile,
                    ecosystem: "compose-dockerfile".to_string(),
                    entrypoint_kind: "service".to_string(),
                });
        }
    }

    for service in &services {
        let Some(service_id) = service_nodes.get(&service.name).copied() else {
            continue;
        };
        for environment in &service.environment {
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "compose_environment".to_string());
            metadata.insert("source".to_string(), "compose".to_string());
            metadata.insert("ecosystem".to_string(), "docker-compose".to_string());
            metadata.insert("service".to_string(), service.name.clone());
            metadata.insert("line".to_string(), environment.line.to_string());
            metadata.insert(
                "value_present".to_string(),
                environment.value_present.to_string(),
            );
            if !environment.value_present {
                metadata.insert("value_source".to_string(), "host".to_string());
            }
            let environment_id = context.graph.add_node_with_metadata(
                NodeKind::Environment,
                environment.name.clone(),
                Some(line_span(label, source, environment.line)),
                metadata,
            );
            let mut edge_metadata = BTreeMap::new();
            edge_metadata.insert("source".to_string(), "compose".to_string());
            edge_metadata.insert("relation".to_string(), "compose_environment".to_string());
            edge_metadata.insert("service".to_string(), service.name.clone());
            add_edge_once_with_metadata(
                context,
                service_id,
                environment_id,
                EdgeKind::ReadsEnvironment,
                Confidence::Exact,
                edge_metadata,
            );
        }

        for env_file in &service.env_files {
            let Some(normalized_env_file) = normalize_manifest_relative_path(label, &env_file.path)
            else {
                continue;
            };
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "compose_env_file".to_string());
            metadata.insert("source".to_string(), "compose".to_string());
            metadata.insert("ecosystem".to_string(), "docker-compose".to_string());
            metadata.insert("service".to_string(), service.name.clone());
            metadata.insert("line".to_string(), env_file.line.to_string());
            metadata.insert("env_file".to_string(), env_file.path.clone());
            metadata.insert("env_file_path".to_string(), normalized_env_file.clone());
            let config_id = context.graph.add_node_with_metadata(
                NodeKind::Config,
                format!("compose env file:{normalized_env_file}"),
                Some(line_span(label, source, env_file.line)),
                metadata,
            );
            let mut edge_metadata = BTreeMap::new();
            edge_metadata.insert("source".to_string(), "compose".to_string());
            edge_metadata.insert("relation".to_string(), "compose_env_file".to_string());
            edge_metadata.insert("service".to_string(), service.name.clone());
            add_edge_once_with_metadata(
                context,
                service_id,
                config_id,
                EdgeKind::ReadsConfig,
                Confidence::Exact,
                edge_metadata,
            );
            context
                .pending_compose_config_targets
                .push(PendingComposeConfigTarget {
                    config: config_id,
                    manifest_label: label.to_string(),
                    target: env_file.path.clone(),
                });
        }

        for port in &service.ports {
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "compose_port".to_string());
            metadata.insert("source".to_string(), "compose".to_string());
            metadata.insert("ecosystem".to_string(), "docker-compose".to_string());
            metadata.insert("service".to_string(), service.name.clone());
            metadata.insert("line".to_string(), port.line.to_string());
            metadata.insert("protocol".to_string(), port.protocol.clone());
            if let Some(raw) = port.raw.as_deref() {
                metadata.insert("raw".to_string(), raw.to_string());
            }
            if let Some(published) = port.published.as_deref() {
                metadata.insert("published_port".to_string(), published.to_string());
            }
            if let Some(target) = port.target.as_deref() {
                metadata.insert("target_port".to_string(), target.to_string());
            }
            if let Some(host_ip) = port.host_ip.as_deref() {
                metadata.insert("host_ip".to_string(), host_ip.to_string());
            }
            let port_id = context.graph.add_node_with_metadata(
                NodeKind::Config,
                compose_port_label(port),
                Some(line_span(label, source, port.line)),
                metadata,
            );
            let mut edge_metadata = BTreeMap::new();
            edge_metadata.insert("source".to_string(), "compose".to_string());
            edge_metadata.insert("relation".to_string(), "compose_port".to_string());
            edge_metadata.insert("service".to_string(), service.name.clone());
            add_edge_once_with_metadata(
                context,
                service_id,
                port_id,
                EdgeKind::References,
                Confidence::Exact,
                edge_metadata,
            );
        }

        for volume in &service.volumes {
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "compose_volume".to_string());
            metadata.insert("source".to_string(), "compose".to_string());
            metadata.insert("ecosystem".to_string(), "docker-compose".to_string());
            metadata.insert("service".to_string(), service.name.clone());
            metadata.insert("line".to_string(), volume.line.to_string());
            metadata.insert("volume_kind".to_string(), volume.kind.clone());
            metadata.insert("read_only".to_string(), volume.read_only.to_string());
            if let Some(raw) = volume.raw.as_deref() {
                metadata.insert("raw".to_string(), raw.to_string());
            }
            if let Some(source) = volume.source.as_deref() {
                metadata.insert("source_path".to_string(), source.to_string());
                if let Some(local_source_path) = compose_volume_local_source_path(label, source) {
                    metadata.insert("local_source_path".to_string(), local_source_path);
                }
            }
            if let Some(target) = volume.target.as_deref() {
                metadata.insert("target_path".to_string(), target.to_string());
            }
            let volume_id = context.graph.add_node_with_metadata(
                NodeKind::Config,
                compose_volume_label(volume),
                Some(line_span(label, source, volume.line)),
                metadata.clone(),
            );
            let mut edge_metadata = BTreeMap::new();
            edge_metadata.insert("source".to_string(), "compose".to_string());
            edge_metadata.insert("relation".to_string(), "compose_volume".to_string());
            edge_metadata.insert("service".to_string(), service.name.clone());
            add_edge_once_with_metadata(
                context,
                service_id,
                volume_id,
                EdgeKind::References,
                Confidence::Exact,
                edge_metadata,
            );
            if let Some(source) = volume.source.as_deref()
                && compose_volume_local_source_path(label, source).is_some()
            {
                context
                    .pending_compose_volume_targets
                    .push(PendingComposeVolumeTarget {
                        volume: volume_id,
                        manifest_label: label.to_string(),
                        target: source.to_string(),
                    });
            }
        }
    }

    for service in &services {
        let Some(source_id) = service_nodes.get(&service.name).copied() else {
            continue;
        };
        for dependency in &service.depends_on {
            let Some(target_id) = service_nodes.get(dependency).copied() else {
                continue;
            };
            let mut metadata = BTreeMap::new();
            metadata.insert(
                "relation".to_string(),
                "compose_service_depends_on".to_string(),
            );
            metadata.insert("source".to_string(), "compose".to_string());
            metadata.insert("service".to_string(), service.name.clone());
            metadata.insert("dependency".to_string(), dependency.clone());
            add_edge_once_with_metadata(
                context,
                source_id,
                target_id,
                EdgeKind::DependsOn,
                Confidence::Exact,
                metadata,
            );
        }
    }
}

pub(crate) fn index_github_actions_workflow_entrypoints(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_github_actions_workflow_path(label, path) {
        return;
    }
    let workflow = github_actions_workflow(label, source);
    if workflow.jobs.is_empty() {
        return;
    }

    let mut job_nodes = BTreeMap::new();
    for job in &workflow.jobs {
        let uses_count = job.steps.iter().filter(|step| step.uses.is_some()).count();
        let run_count = job.steps.iter().filter(|step| step.run.is_some()).count();
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "github_actions_job".to_string());
        metadata.insert("entrypoint_kind".to_string(), "workflow_job".to_string());
        metadata.insert("ecosystem".to_string(), "github-actions".to_string());
        metadata.insert("source".to_string(), "github-actions".to_string());
        metadata.insert("workflow".to_string(), workflow.name.clone());
        metadata.insert("job".to_string(), job.id.clone());
        metadata.insert("line".to_string(), job.line.to_string());
        metadata.insert("step_count".to_string(), job.steps.len().to_string());
        metadata.insert("uses_count".to_string(), uses_count.to_string());
        metadata.insert("run_count".to_string(), run_count.to_string());
        metadata.insert("needs_count".to_string(), job.needs.len().to_string());
        let environment_count = workflow.environment.len() + job.environment.len();
        metadata.insert(
            "environment_count".to_string(),
            environment_count.to_string(),
        );
        if !job.needs.is_empty() {
            metadata.insert("needs".to_string(), job.needs.join(","));
        }
        if let Some(display_name) = job.display_name.as_deref() {
            metadata.insert("name".to_string(), display_name.to_string());
        }
        if let Some(runs_on) = job.runs_on.as_deref() {
            metadata.insert("runs_on".to_string(), runs_on.to_string());
        }

        let job_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("github workflow:{}/{}", workflow.name, job.id),
            Some(block_span(label, source, job.line, job.end_line)),
            metadata,
        );
        job_nodes.insert(job.id.clone(), job_id);
        add_edge_once(
            context,
            file_id,
            job_id,
            EdgeKind::Contains,
            Confidence::Exact,
        );
        let root_id = context.graph.root;
        add_edge_once(
            context,
            root_id,
            job_id,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        add_entrypoint_reference(
            context,
            job_id,
            file_id,
            "entrypoint_file",
            "github_actions_workflow",
            Confidence::Exact,
            None,
        );

        for step in &job.steps {
            index_github_actions_step(context, job_id, label, source, &workflow, job, step);
        }
        for environment in workflow.environment.iter().chain(job.environment.iter()) {
            index_ci_environment(
                context,
                job_id,
                label,
                source,
                "github-actions",
                &job.id,
                environment,
            );
        }
    }

    for job in &workflow.jobs {
        let Some(source_id) = job_nodes.get(&job.id).copied() else {
            continue;
        };
        for dependency in &job.needs {
            let Some(target_id) = job_nodes.get(dependency).copied() else {
                continue;
            };
            let mut metadata = BTreeMap::new();
            metadata.insert("relation".to_string(), "github_actions_needs".to_string());
            metadata.insert("source".to_string(), "github-actions".to_string());
            metadata.insert("workflow".to_string(), workflow.name.clone());
            metadata.insert("job".to_string(), job.id.clone());
            metadata.insert("dependency".to_string(), dependency.clone());
            add_edge_once_with_metadata(
                context,
                source_id,
                target_id,
                EdgeKind::DependsOn,
                Confidence::Exact,
                metadata,
            );
        }
    }
}

/// One workflow/job/step position inside a GitHub Actions file, bundled so
/// step indexers take one scope instead of three parallel parameters.
pub(crate) struct GithubActionsStepScope<'a> {
    workflow: &'a GithubActionsWorkflow,
    job: &'a GithubActionsJob,
    step: &'a GithubActionsStep,
}

pub(crate) fn index_github_actions_step(
    context: &mut IndexContext,
    job_id: NodeId,
    label: &str,
    source: &str,
    workflow: &GithubActionsWorkflow,
    job: &GithubActionsJob,
    step: &GithubActionsStep,
) {
    let scope = GithubActionsStepScope {
        workflow,
        job,
        step,
    };
    if let Some(action) = step.uses.as_deref() {
        index_github_actions_uses_step(context, job_id, label, source, &scope, action);
    }
    if let Some(command) = step.run.as_deref() {
        index_github_actions_run_step(context, job_id, label, source, &scope, command);
    }
}

pub(crate) fn index_github_actions_uses_step(
    context: &mut IndexContext,
    job_id: NodeId,
    label: &str,
    source: &str,
    scope: &GithubActionsStepScope<'_>,
    action: &str,
) {
    let GithubActionsStepScope {
        workflow,
        job,
        step,
    } = scope;
    if let Some(local_path) = github_actions_local_action_path(action) {
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "item_kind".to_string(),
            "github_actions_local_action".to_string(),
        );
        metadata.insert("source".to_string(), "github-actions".to_string());
        metadata.insert("ecosystem".to_string(), "github-actions".to_string());
        metadata.insert("workflow".to_string(), workflow.name.clone());
        metadata.insert("job".to_string(), job.id.clone());
        metadata.insert("line".to_string(), step.line.to_string());
        metadata.insert("uses".to_string(), action.to_string());
        metadata.insert("local_action_path".to_string(), local_path.clone());
        if let Some(name) = step.name.as_deref() {
            metadata.insert("name".to_string(), name.to_string());
        }
        let action_id = context.graph.add_node_with_metadata(
            NodeKind::Config,
            format!("github action:{local_path}"),
            Some(line_span(label, source, step.line)),
            metadata,
        );
        add_github_actions_uses_edge(context, job_id, action_id, workflow, job, action);
        context
            .pending_github_actions_local_actions
            .push(PendingGithubActionsLocalAction {
                action: action_id,
                target: local_path,
            });
        return;
    }

    let (name, version) = github_actions_remote_action(action);
    let action_id = github_actions_external_action_node(context, &name, version.as_deref());
    add_github_actions_uses_edge(context, job_id, action_id, workflow, job, action);
}

pub(crate) fn index_github_actions_run_step(
    context: &mut IndexContext,
    job_id: NodeId,
    label: &str,
    source: &str,
    scope: &GithubActionsStepScope<'_>,
    command: &str,
) {
    let GithubActionsStepScope {
        workflow,
        job,
        step,
    } = scope;
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "item_kind".to_string(),
        "github_actions_run_step".to_string(),
    );
    metadata.insert("source".to_string(), "github-actions".to_string());
    metadata.insert("ecosystem".to_string(), "github-actions".to_string());
    metadata.insert("workflow".to_string(), workflow.name.clone());
    metadata.insert("job".to_string(), job.id.clone());
    metadata.insert("line".to_string(), step.line.to_string());
    metadata.insert("command".to_string(), command.to_string());
    if let Some(command_path) = root_relative_command_path_candidate(command) {
        metadata.insert("command_path".to_string(), command_path);
    }
    if let Some(name) = step.name.as_deref() {
        metadata.insert("name".to_string(), name.to_string());
    }
    let step_id = context.graph.add_node_with_metadata(
        NodeKind::Config,
        format!("github run:{}/{}/{}", workflow.name, job.id, step.line),
        Some(line_span(label, source, step.line)),
        metadata,
    );
    let mut edge_metadata = BTreeMap::new();
    edge_metadata.insert("source".to_string(), "github-actions".to_string());
    edge_metadata.insert("relation".to_string(), "github_actions_run".to_string());
    edge_metadata.insert("workflow".to_string(), workflow.name.clone());
    edge_metadata.insert("job".to_string(), job.id.clone());
    add_edge_once_with_metadata(
        context,
        job_id,
        step_id,
        EdgeKind::References,
        Confidence::Exact,
        edge_metadata,
    );
    context
        .pending_entrypoint_targets
        .push(PendingEntrypointTarget {
            entrypoint: job_id,
            manifest_label: label.to_string(),
            target: command.to_string(),
            ecosystem: "github-actions".to_string(),
            entrypoint_kind: "workflow_job".to_string(),
        });
}

pub(crate) fn add_github_actions_uses_edge(
    context: &mut IndexContext,
    job_id: NodeId,
    action_id: NodeId,
    workflow: &GithubActionsWorkflow,
    job: &GithubActionsJob,
    action: &str,
) {
    let mut metadata = BTreeMap::new();
    metadata.insert("source".to_string(), "github-actions".to_string());
    metadata.insert("relation".to_string(), "github_actions_uses".to_string());
    metadata.insert("workflow".to_string(), workflow.name.clone());
    metadata.insert("job".to_string(), job.id.clone());
    metadata.insert("uses".to_string(), action.to_string());
    if let Some((_, version)) = action.rsplit_once('@')
        && !version.trim().is_empty()
    {
        metadata.insert("version".to_string(), version.trim().to_string());
    }
    add_edge_once_with_metadata(
        context,
        job_id,
        action_id,
        EdgeKind::DependsOn,
        Confidence::Exact,
        metadata,
    );
}

pub(crate) fn github_actions_external_action_node(
    context: &mut IndexContext,
    action: &str,
    version: Option<&str>,
) -> NodeId {
    let package_id = format!("github-actions:{action}");
    if let Some(id) = context.external_dependencies.get(&package_id).copied() {
        return id;
    }
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "dependency".to_string());
    metadata.insert("ecosystem".to_string(), "github-actions".to_string());
    metadata.insert("package_id".to_string(), package_id.clone());
    metadata.insert("source".to_string(), "github-actions".to_string());
    if let Some(version) = version {
        metadata.insert("version".to_string(), version.to_string());
    }
    let id = context.graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        format!("github action:{action}"),
        None,
        metadata,
    );
    context.external_dependencies.insert(package_id, id);
    id
}

pub(crate) fn index_gitlab_ci_entrypoints(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_gitlab_ci_path(label, path) {
        return;
    }
    let jobs = gitlab_ci_jobs(source);
    if jobs.is_empty() {
        return;
    }

    let mut job_nodes = BTreeMap::new();
    for job in &jobs {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "gitlab_ci_job".to_string());
        metadata.insert("entrypoint_kind".to_string(), "pipeline_job".to_string());
        metadata.insert("ecosystem".to_string(), "gitlab-ci".to_string());
        metadata.insert("source".to_string(), "gitlab-ci".to_string());
        metadata.insert("job".to_string(), job.name.clone());
        metadata.insert("line".to_string(), job.line.to_string());
        metadata.insert("script_count".to_string(), job.scripts.len().to_string());
        metadata.insert("needs_count".to_string(), job.needs.len().to_string());
        metadata.insert(
            "dependencies_count".to_string(),
            job.dependencies.len().to_string(),
        );
        metadata.insert(
            "environment_count".to_string(),
            job.variables.len().to_string(),
        );
        if !job.needs.is_empty() {
            metadata.insert("needs".to_string(), job.needs.join(","));
        }
        if !job.dependencies.is_empty() {
            metadata.insert("dependencies".to_string(), job.dependencies.join(","));
        }
        if let Some(stage) = job.stage.as_deref() {
            metadata.insert("stage".to_string(), stage.to_string());
        }
        if let Some(image) = job.image.as_deref() {
            metadata.insert("image".to_string(), image.to_string());
        }
        if !job.extends.is_empty() {
            metadata.insert("extends".to_string(), job.extends.join(","));
        }

        let job_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("gitlab job:{}", job.name),
            Some(line_span(label, source, job.line)),
            metadata,
        );
        job_nodes.insert(job.name.clone(), job_id);
        add_edge_once(
            context,
            file_id,
            job_id,
            EdgeKind::Contains,
            Confidence::Exact,
        );
        let root_id = context.graph.root;
        add_edge_once(
            context,
            root_id,
            job_id,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        add_entrypoint_reference(
            context,
            job_id,
            file_id,
            "entrypoint_file",
            "gitlab_ci_config",
            Confidence::Exact,
            None,
        );

        for script in &job.scripts {
            index_gitlab_ci_script(context, job_id, label, source, job, script);
        }
        for variable in &job.variables {
            index_ci_environment(
                context,
                job_id,
                label,
                source,
                "gitlab-ci",
                &job.name,
                variable,
            );
        }
    }

    for job in &jobs {
        let Some(source_id) = job_nodes.get(&job.name).copied() else {
            continue;
        };
        for dependency in &job.needs {
            add_gitlab_ci_job_dependency_edge(
                context,
                source_id,
                &job_nodes,
                job,
                dependency,
                "gitlab_ci_needs",
            );
        }
        for dependency in &job.dependencies {
            add_gitlab_ci_job_dependency_edge(
                context,
                source_id,
                &job_nodes,
                job,
                dependency,
                "gitlab_ci_dependencies",
            );
        }
    }
}

pub(crate) fn index_gitlab_ci_script(
    context: &mut IndexContext,
    job_id: NodeId,
    label: &str,
    source: &str,
    job: &GitlabCiJob,
    script: &GitlabCiScript,
) {
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "gitlab_ci_script".to_string());
    metadata.insert("source".to_string(), "gitlab-ci".to_string());
    metadata.insert("ecosystem".to_string(), "gitlab-ci".to_string());
    metadata.insert("job".to_string(), job.name.clone());
    metadata.insert("line".to_string(), script.line.to_string());
    metadata.insert("ordinal".to_string(), script.ordinal.to_string());
    metadata.insert("script_kind".to_string(), script.script_kind.clone());
    metadata.insert("command".to_string(), script.command.clone());
    if let Some(stage) = job.stage.as_deref() {
        metadata.insert("stage".to_string(), stage.to_string());
    }
    if let Some(command_path) = root_relative_command_path_candidate(&script.command) {
        metadata.insert("command_path".to_string(), command_path);
    }
    let script_id = context.graph.add_node_with_metadata(
        NodeKind::Config,
        format!(
            "gitlab script:{}/{}#{}",
            job.name, script.line, script.ordinal
        ),
        Some(line_span(label, source, script.line)),
        metadata,
    );
    let mut edge_metadata = BTreeMap::new();
    edge_metadata.insert("source".to_string(), "gitlab-ci".to_string());
    edge_metadata.insert("relation".to_string(), "gitlab_ci_script".to_string());
    edge_metadata.insert("job".to_string(), job.name.clone());
    edge_metadata.insert("script_kind".to_string(), script.script_kind.clone());
    add_edge_once_with_metadata(
        context,
        job_id,
        script_id,
        EdgeKind::References,
        Confidence::Exact,
        edge_metadata,
    );
    context
        .pending_entrypoint_targets
        .push(PendingEntrypointTarget {
            entrypoint: job_id,
            manifest_label: label.to_string(),
            target: script.command.clone(),
            ecosystem: "gitlab-ci".to_string(),
            entrypoint_kind: "pipeline_job".to_string(),
        });
}

pub(crate) fn index_ci_environment(
    context: &mut IndexContext,
    job_id: NodeId,
    label: &str,
    source: &str,
    ci_source: &str,
    job_name: &str,
    environment: &CiEnvironment,
) {
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "ci_environment".to_string());
    metadata.insert("source".to_string(), ci_source.to_string());
    metadata.insert("ecosystem".to_string(), ci_source.to_string());
    metadata.insert("job".to_string(), job_name.to_string());
    metadata.insert("scope".to_string(), environment.scope.clone());
    metadata.insert("line".to_string(), environment.line.to_string());
    metadata.insert(
        "value_present".to_string(),
        environment.value_present.to_string(),
    );
    metadata.insert("value_kind".to_string(), environment.value_kind.clone());
    if !environment.value_present {
        metadata.insert("value_source".to_string(), "runner".to_string());
    }
    let environment_id = context.graph.add_node_with_metadata(
        NodeKind::Environment,
        environment.name.clone(),
        Some(line_span(label, source, environment.line)),
        metadata,
    );
    let mut edge_metadata = BTreeMap::new();
    edge_metadata.insert("source".to_string(), ci_source.to_string());
    edge_metadata.insert("relation".to_string(), "ci_environment".to_string());
    edge_metadata.insert("job".to_string(), job_name.to_string());
    edge_metadata.insert("scope".to_string(), environment.scope.clone());
    add_edge_once_with_metadata(
        context,
        job_id,
        environment_id,
        EdgeKind::ReadsEnvironment,
        Confidence::Exact,
        edge_metadata,
    );
}

pub(crate) fn add_gitlab_ci_job_dependency_edge(
    context: &mut IndexContext,
    source_id: NodeId,
    job_nodes: &BTreeMap<String, NodeId>,
    job: &GitlabCiJob,
    dependency: &str,
    relation: &str,
) {
    let Some(target_id) = job_nodes.get(dependency).copied() else {
        return;
    };
    let mut metadata = BTreeMap::new();
    metadata.insert("relation".to_string(), relation.to_string());
    metadata.insert("source".to_string(), "gitlab-ci".to_string());
    metadata.insert("job".to_string(), job.name.clone());
    metadata.insert("dependency".to_string(), dependency.to_string());
    add_edge_once_with_metadata(
        context,
        source_id,
        target_id,
        EdgeKind::DependsOn,
        Confidence::Exact,
        metadata,
    );
}

pub(crate) fn index_kubernetes_manifest_facts(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_kubernetes_manifest_candidate(path, source) {
        return;
    }
    let documents = kubernetes_documents(source);
    if documents.is_empty() {
        return;
    }

    let mut service_nodes = Vec::new();
    let mut workload_nodes = Vec::new();
    for document in &documents {
        if let Some(config_kind) = kubernetes_config_kind(&document.kind) {
            let key = KubernetesConfigKey {
                namespace: document.namespace.clone(),
                config_kind: config_kind.to_string(),
                name: document.name.clone(),
            };
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "kubernetes_config".to_string());
            metadata.insert("source".to_string(), "kubernetes".to_string());
            metadata.insert("ecosystem".to_string(), "kubernetes".to_string());
            metadata.insert("kubernetes_kind".to_string(), document.kind.clone());
            metadata.insert("config_kind".to_string(), config_kind.to_string());
            metadata.insert("name".to_string(), document.name.clone());
            metadata.insert("namespace".to_string(), document.namespace.clone());
            metadata.insert("line".to_string(), document.line.to_string());
            let config_id = context.graph.add_node_with_metadata(
                NodeKind::Config,
                kubernetes_resource_label(config_kind, &document.namespace, &document.name),
                Some(line_span(label, source, document.line)),
                metadata,
            );
            context.kubernetes_configs.insert(key, config_id);
            add_edge_once(
                context,
                file_id,
                config_id,
                EdgeKind::Contains,
                Confidence::Exact,
            );
            continue;
        }

        if document.kind == "Service" {
            let service_id = index_kubernetes_service(context, file_id, label, source, document);
            context.kubernetes_services.insert(
                KubernetesServiceKey {
                    namespace: document.namespace.clone(),
                    name: document.name.clone(),
                },
                service_id,
            );
            service_nodes.push((service_id, document));
            continue;
        }

        if document.kind == "Ingress" {
            index_kubernetes_ingress(context, file_id, label, source, document);
            continue;
        }

        if kubernetes_workload_kind(&document.kind) {
            let workload_id = index_kubernetes_workload(context, file_id, label, source, document);
            workload_nodes.push((workload_id, document));
        }
    }

    link_kubernetes_services_to_workloads(context, &service_nodes, &workload_nodes);
}

pub(crate) fn index_kubernetes_service(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
    document: &KubernetesDocument,
) -> NodeId {
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "kubernetes_service".to_string());
    metadata.insert("source".to_string(), "kubernetes".to_string());
    metadata.insert("ecosystem".to_string(), "kubernetes".to_string());
    metadata.insert("kubernetes_kind".to_string(), document.kind.clone());
    metadata.insert("name".to_string(), document.name.clone());
    metadata.insert("namespace".to_string(), document.namespace.clone());
    metadata.insert("line".to_string(), document.line.to_string());
    metadata.insert(
        "port_count".to_string(),
        document.service_ports.len().to_string(),
    );
    if !document.selector_labels.is_empty() {
        metadata.insert(
            "selector".to_string(),
            kubernetes_label_selector_string(&document.selector_labels),
        );
    }
    let service_id = context.graph.add_node_with_metadata(
        NodeKind::Config,
        kubernetes_resource_label("service", &document.namespace, &document.name),
        Some(line_span(label, source, document.line)),
        metadata,
    );
    add_edge_once(
        context,
        file_id,
        service_id,
        EdgeKind::Contains,
        Confidence::Exact,
    );

    for port in &document.service_ports {
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "item_kind".to_string(),
            "kubernetes_service_port".to_string(),
        );
        metadata.insert("source".to_string(), "kubernetes".to_string());
        metadata.insert("ecosystem".to_string(), "kubernetes".to_string());
        metadata.insert("service".to_string(), document.name.clone());
        metadata.insert("namespace".to_string(), document.namespace.clone());
        metadata.insert("protocol".to_string(), port.protocol.clone());
        metadata.insert("line".to_string(), port.line.to_string());
        if let Some(name) = port.name.as_deref() {
            metadata.insert("name".to_string(), name.to_string());
        }
        if let Some(value) = port.port.as_deref() {
            metadata.insert("port".to_string(), value.to_string());
        }
        if let Some(value) = port.target_port.as_deref() {
            metadata.insert("target_port".to_string(), value.to_string());
        }
        let port_id = context.graph.add_node_with_metadata(
            NodeKind::Config,
            kubernetes_service_port_label(document, port),
            Some(line_span(label, source, port.line)),
            metadata,
        );
        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("source".to_string(), "kubernetes".to_string());
        edge_metadata.insert(
            "relation".to_string(),
            "kubernetes_service_port".to_string(),
        );
        add_edge_once_with_metadata(
            context,
            service_id,
            port_id,
            EdgeKind::References,
            Confidence::Exact,
            edge_metadata,
        );
    }
    service_id
}

pub(crate) fn index_kubernetes_ingress(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
    document: &KubernetesDocument,
) -> NodeId {
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "kubernetes_ingress".to_string());
    metadata.insert("entrypoint_kind".to_string(), "ingress".to_string());
    metadata.insert("source".to_string(), "kubernetes".to_string());
    metadata.insert("ecosystem".to_string(), "kubernetes".to_string());
    metadata.insert("kubernetes_kind".to_string(), document.kind.clone());
    metadata.insert("name".to_string(), document.name.clone());
    metadata.insert("namespace".to_string(), document.namespace.clone());
    metadata.insert("line".to_string(), document.line.to_string());
    metadata.insert(
        "backend_count".to_string(),
        document.ingress_backends.len().to_string(),
    );
    let ingress_id = context.graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        kubernetes_resource_label("ingress", &document.namespace, &document.name),
        Some(line_span(label, source, document.line)),
        metadata,
    );
    add_edge_once(
        context,
        file_id,
        ingress_id,
        EdgeKind::Contains,
        Confidence::Exact,
    );
    let root_id = context.graph.root;
    add_edge_once(
        context,
        root_id,
        ingress_id,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    let mut edge_metadata = BTreeMap::new();
    edge_metadata.insert("relation".to_string(), "entrypoint_file".to_string());
    edge_metadata.insert("resolution".to_string(), "kubernetes_manifest".to_string());
    edge_metadata.insert("source".to_string(), "kubernetes".to_string());
    add_edge_once_with_metadata(
        context,
        ingress_id,
        file_id,
        EdgeKind::References,
        Confidence::Exact,
        edge_metadata,
    );

    for backend in &document.ingress_backends {
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "item_kind".to_string(),
            "kubernetes_service_ref".to_string(),
        );
        metadata.insert("source".to_string(), "kubernetes".to_string());
        metadata.insert("ecosystem".to_string(), "kubernetes".to_string());
        metadata.insert("ref_kind".to_string(), "ingress_backend".to_string());
        metadata.insert("name".to_string(), backend.service_name.clone());
        metadata.insert("namespace".to_string(), document.namespace.clone());
        metadata.insert("ingress".to_string(), document.name.clone());
        metadata.insert("line".to_string(), backend.line.to_string());
        if let Some(port) = backend.service_port.as_deref() {
            metadata.insert("service_port".to_string(), port.to_string());
        }
        if let Some(host) = backend.host.as_deref() {
            metadata.insert("host".to_string(), host.to_string());
        }
        if let Some(path) = backend.path.as_deref() {
            metadata.insert("path".to_string(), path.to_string());
        }
        if let Some(path_type) = backend.path_type.as_deref() {
            metadata.insert("path_type".to_string(), path_type.to_string());
        }
        let service_ref_id = context.graph.add_node_with_metadata(
            NodeKind::Config,
            kubernetes_service_ref_label(&document.namespace, &backend.service_name),
            Some(line_span(label, source, backend.line)),
            metadata,
        );
        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("source".to_string(), "kubernetes".to_string());
        edge_metadata.insert(
            "relation".to_string(),
            "kubernetes_ingress_backend".to_string(),
        );
        if let Some(path) = backend.path.as_deref() {
            edge_metadata.insert("path".to_string(), path.to_string());
        }
        if let Some(host) = backend.host.as_deref() {
            edge_metadata.insert("host".to_string(), host.to_string());
        }
        add_edge_once_with_metadata(
            context,
            ingress_id,
            service_ref_id,
            EdgeKind::References,
            Confidence::Exact,
            edge_metadata,
        );
        context
            .pending_kubernetes_service_refs
            .push(PendingKubernetesServiceRef {
                service_ref: service_ref_id,
                namespace: document.namespace.clone(),
                name: backend.service_name.clone(),
            });
    }

    ingress_id
}

pub(crate) fn index_kubernetes_workload(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
    document: &KubernetesDocument,
) -> NodeId {
    let kind_slug = document.kind.to_ascii_lowercase();
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "kubernetes_workload".to_string());
    metadata.insert("entrypoint_kind".to_string(), "workload".to_string());
    metadata.insert("source".to_string(), "kubernetes".to_string());
    metadata.insert("ecosystem".to_string(), "kubernetes".to_string());
    metadata.insert("kubernetes_kind".to_string(), document.kind.clone());
    metadata.insert("name".to_string(), document.name.clone());
    metadata.insert("namespace".to_string(), document.namespace.clone());
    metadata.insert("line".to_string(), document.line.to_string());
    metadata.insert(
        "config_ref_count".to_string(),
        document.config_refs.len().to_string(),
    );
    metadata.insert(
        "container_count".to_string(),
        document.container_count.to_string(),
    );
    if !document.labels.is_empty() {
        metadata.insert(
            "labels".to_string(),
            kubernetes_label_selector_string(&document.labels),
        );
    }
    if !document.pod_labels.is_empty() {
        metadata.insert(
            "pod_labels".to_string(),
            kubernetes_label_selector_string(&document.pod_labels),
        );
    }
    let workload_id = context.graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        kubernetes_resource_label(&kind_slug, &document.namespace, &document.name),
        Some(line_span(label, source, document.line)),
        metadata,
    );
    add_edge_once(
        context,
        file_id,
        workload_id,
        EdgeKind::Contains,
        Confidence::Exact,
    );
    let root_id = context.graph.root;
    add_edge_once(
        context,
        root_id,
        workload_id,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    let mut edge_metadata = BTreeMap::new();
    edge_metadata.insert("relation".to_string(), "entrypoint_file".to_string());
    edge_metadata.insert("resolution".to_string(), "kubernetes_manifest".to_string());
    edge_metadata.insert("source".to_string(), "kubernetes".to_string());
    add_edge_once_with_metadata(
        context,
        workload_id,
        file_id,
        EdgeKind::References,
        Confidence::Exact,
        edge_metadata,
    );

    for config_ref in &document.config_refs {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "kubernetes_config_ref".to_string());
        metadata.insert("source".to_string(), "kubernetes".to_string());
        metadata.insert("ecosystem".to_string(), "kubernetes".to_string());
        metadata.insert("config_kind".to_string(), config_ref.config_kind.clone());
        metadata.insert("ref_kind".to_string(), config_ref.ref_kind.clone());
        metadata.insert("name".to_string(), config_ref.name.clone());
        metadata.insert("namespace".to_string(), document.namespace.clone());
        metadata.insert("workload".to_string(), document.name.clone());
        metadata.insert("workload_kind".to_string(), document.kind.clone());
        metadata.insert("line".to_string(), config_ref.line.to_string());
        let config_ref_id = context.graph.add_node_with_metadata(
            NodeKind::Config,
            kubernetes_config_ref_label(
                &config_ref.config_kind,
                &document.namespace,
                &config_ref.name,
            ),
            Some(line_span(label, source, config_ref.line)),
            metadata,
        );
        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("source".to_string(), "kubernetes".to_string());
        edge_metadata.insert("relation".to_string(), "kubernetes_config_ref".to_string());
        edge_metadata.insert("ref_kind".to_string(), config_ref.ref_kind.clone());
        add_edge_once_with_metadata(
            context,
            workload_id,
            config_ref_id,
            EdgeKind::ReadsConfig,
            Confidence::Exact,
            edge_metadata,
        );
        context
            .pending_kubernetes_config_refs
            .push(PendingKubernetesConfigRef {
                config_ref: config_ref_id,
                namespace: document.namespace.clone(),
                config_kind: config_ref.config_kind.clone(),
                name: config_ref.name.clone(),
            });
    }
    workload_id
}

pub(crate) fn link_kubernetes_services_to_workloads(
    context: &mut IndexContext,
    service_nodes: &[(NodeId, &KubernetesDocument)],
    workload_nodes: &[(NodeId, &KubernetesDocument)],
) {
    for (service_id, service) in service_nodes {
        if service.selector_labels.is_empty() {
            continue;
        }
        for (workload_id, workload) in workload_nodes {
            if service.namespace != workload.namespace {
                continue;
            }
            let workload_labels = kubernetes_workload_match_labels(workload);
            if !kubernetes_selector_matches(&service.selector_labels, workload_labels) {
                continue;
            }
            let mut metadata = BTreeMap::new();
            metadata.insert("source".to_string(), "kubernetes".to_string());
            metadata.insert(
                "relation".to_string(),
                "kubernetes_service_selector".to_string(),
            );
            metadata.insert(
                "selector".to_string(),
                kubernetes_label_selector_string(&service.selector_labels),
            );
            metadata.insert("service".to_string(), service.name.clone());
            metadata.insert("workload".to_string(), workload.name.clone());
            metadata.insert("namespace".to_string(), service.namespace.clone());
            add_edge_once_with_metadata(
                context,
                *service_id,
                *workload_id,
                EdgeKind::References,
                Confidence::Exact,
                metadata,
            );
        }
    }
}

pub(crate) fn manifest_dependencies(
    path: &Path,
    source: &str,
    cargo_workspace_dependencies: &BTreeMap<String, Option<String>>,
) -> Vec<ManifestDependency> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("Cargo.toml") => cargo_dependencies(source, cargo_workspace_dependencies),
        Some("package.json") => package_json_dependencies(source),
        Some("package-lock.json") => package_lock_dependencies(source),
        Some("pnpm-lock.yaml") => pnpm_lock_dependencies(source),
        Some("go.mod") => go_mod_dependencies(source),
        Some("requirements.txt") => requirements_dependencies(source),
        Some("pyproject.toml") => pyproject_dependencies(source),
        Some("setup.py") => setup_py_dependencies(source),
        Some("setup.cfg") => setup_cfg_dependencies(source),
        Some("Pipfile") => pipfile_dependencies(source),
        Some("composer.json") => composer_dependencies(source),
        Some("composer.lock") => composer_lock_dependencies(source),
        Some("vcpkg.json") => vcpkg_dependencies(source),
        Some("conanfile.txt") => conanfile_txt_dependencies(source),
        Some("CMakeLists.txt") => cmake_dependencies(source),
        Some("pubspec.yaml") => pubspec_dependencies(source),
        _ => Vec::new(),
    }
}

pub(crate) fn manifest_entrypoints(path: &Path, source: &str) -> Vec<ManifestEntrypoint> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("Cargo.toml") => cargo_entrypoints(path, source),
        Some("package.json") => package_json_entrypoints(source),
        Some("go.mod") => go_mod_entrypoints(path, source),
        Some("pyproject.toml") => pyproject_entrypoints(source),
        Some("setup.py") => setup_py_entrypoints(source),
        Some("setup.cfg") => setup_cfg_entrypoints(source),
        Some("composer.json") => composer_entrypoints(source),
        Some("CMakeLists.txt") => cmake_entrypoints(source),
        Some("pubspec.yaml") => pubspec_entrypoints(path, source),
        _ => Vec::new(),
    }
}

pub(crate) fn is_makefile_path(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("Makefile" | "makefile" | "GNUmakefile")
    )
}

pub(crate) fn is_dockerfile_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "Dockerfile" || name.ends_with(".Dockerfile"))
}

pub(crate) fn is_compose_file_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                "docker-compose.yml"
                    | "docker-compose.yaml"
                    | "compose.yml"
                    | "compose.yaml"
                    | "docker-compose.override.yml"
                    | "docker-compose.override.yaml"
            ) || name.ends_with(".compose.yml")
                || name.ends_with(".compose.yaml")
        })
}

pub(crate) fn is_kubernetes_manifest_candidate(path: &Path, source: &str) -> bool {
    let yaml_path = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "yaml" | "yml"));
    yaml_path && source.contains("apiVersion:") && source.contains("kind:")
}

pub(crate) fn kubernetes_documents(source: &str) -> Vec<KubernetesDocument> {
    let mut documents = Vec::new();
    let mut start_line = 1u32;
    let mut lines = Vec::new();

    for (index, raw_line) in source.lines().enumerate() {
        if raw_line.trim() == "---" {
            if let Some(document) = kubernetes_document_from_lines(&lines, start_line) {
                documents.push(document);
            }
            lines.clear();
            start_line = index as u32 + 2;
            continue;
        }
        lines.push(raw_line.to_string());
    }

    if let Some(document) = kubernetes_document_from_lines(&lines, start_line) {
        documents.push(document);
    }
    documents
}

pub(crate) fn kubernetes_document_from_lines(
    lines: &[String],
    start_line: u32,
) -> Option<KubernetesDocument> {
    let mut has_api_version = false;
    let mut kind = None;
    let mut metadata_indent = None;
    let mut name = None;
    let mut namespace = None;
    let mut labels = BTreeMap::new();
    let mut pod_labels = BTreeMap::new();
    let mut selector_labels = BTreeMap::new();
    let mut config_refs = Vec::new();
    let mut service_ports = Vec::new();
    let mut ingress_backends = Vec::new();
    let mut active_ref: Option<(String, String, usize, u32)> = None;
    let mut active_ports_indent = None;
    let mut active_service_port: Option<(KubernetesServicePort, usize)> = None;
    let mut active_ingress_host: Option<String> = None;
    let mut active_ingress_path: Option<String> = None;
    let mut active_ingress_path_type: Option<String> = None;
    let mut active_ingress_backend: Option<(KubernetesIngressBackend, usize)> = None;
    let mut active_ingress_service_indent = None;
    let mut active_containers_indent = None;
    let mut active_template_indent = None;
    let mut active_template_metadata_indent = None;
    let mut active_selector_indent = None;
    let mut active_label_map: Option<(KubernetesLabelTarget, usize)> = None;
    let mut container_count = 0usize;

    for (index, raw_line) in lines.iter().enumerate() {
        let line = start_line + index as u32;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let item_trimmed = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        let indent = yaml_indent(raw_line);

        if let Some((_target, label_indent)) = active_label_map
            && indent <= label_indent
        {
            active_label_map = None;
        } else if let Some((target, _)) = active_label_map
            && let Some((key, value)) = yaml_key_pair(trimmed)
        {
            match target {
                KubernetesLabelTarget::Metadata => {
                    labels.insert(key, value);
                }
                KubernetesLabelTarget::PodTemplate => {
                    pod_labels.insert(key, value);
                }
                KubernetesLabelTarget::Selector => {
                    selector_labels.insert(key, value);
                }
            }
            continue;
        }

        if let Some((_, _, ref_indent, _)) = active_ref.as_ref()
            && indent <= *ref_indent
        {
            active_ref = None;
        }
        if let Some((_, backend_indent)) = active_ingress_backend.as_ref()
            && indent <= *backend_indent
        {
            flush_kubernetes_ingress_backend(&mut ingress_backends, &mut active_ingress_backend);
            active_ingress_service_indent = None;
        }
        if let Some(service_indent) = active_ingress_service_indent
            && indent <= service_indent
        {
            active_ingress_service_indent = None;
        }
        if let Some((config_kind, ref_kind, _, ref_line)) = active_ref.as_ref()
            && let Some(value) = yaml_key_value(trimmed, "name")
        {
            config_refs.push(KubernetesConfigRef {
                config_kind: config_kind.clone(),
                ref_kind: ref_kind.clone(),
                name: value,
                line: *ref_line,
            });
            active_ref = None;
            continue;
        }

        if indent == 0 {
            if yaml_key_value(trimmed, "apiVersion").is_some() {
                has_api_version = true;
            } else if let Some(value) = yaml_key_value(trimmed, "kind") {
                kind = Some(value);
            } else if yaml_key(trimmed).is_some_and(|key| key == "metadata") {
                metadata_indent = Some(indent);
            }
        }

        if let Some(parent_indent) = metadata_indent {
            if indent <= parent_indent && yaml_key(trimmed).is_some_and(|key| key != "metadata") {
                metadata_indent = None;
            } else if indent > parent_indent {
                if let Some(value) = yaml_key_value(trimmed, "name") {
                    name = Some(value);
                } else if let Some(value) = yaml_key_value(trimmed, "namespace") {
                    namespace = Some(value);
                }
            }
        }

        if let Some(template_indent) = active_template_indent
            && indent <= template_indent
        {
            active_template_indent = None;
            active_template_metadata_indent = None;
        }
        if let Some(template_metadata_indent) = active_template_metadata_indent
            && indent <= template_metadata_indent
        {
            active_template_metadata_indent = None;
        }
        if let Some(selector_indent) = active_selector_indent
            && indent <= selector_indent
        {
            active_selector_indent = None;
        }
        if let Some((_, port_indent)) = active_service_port.as_ref()
            && indent <= *port_indent
        {
            flush_kubernetes_service_port(&mut service_ports, &mut active_service_port);
        }
        if let Some(ports_indent) = active_ports_indent
            && indent <= ports_indent
        {
            flush_kubernetes_service_port(&mut service_ports, &mut active_service_port);
            active_ports_indent = None;
        }
        if let Some(containers_indent) = active_containers_indent
            && indent <= containers_indent
        {
            active_containers_indent = None;
        }

        if yaml_key(trimmed).is_some_and(|key| key == "ports") {
            active_ports_indent = Some(indent);
            continue;
        }
        if yaml_key(trimmed).is_some_and(|key| key == "containers") {
            active_containers_indent = Some(indent);
            continue;
        }
        if let Some(value) = yaml_key_value(item_trimmed, "host") {
            active_ingress_host = Some(value);
            continue;
        }
        if let Some(value) = yaml_key_value(item_trimmed, "path") {
            active_ingress_path = Some(value);
            continue;
        }
        if let Some(value) = yaml_key_value(item_trimmed, "pathType") {
            active_ingress_path_type = Some(value);
            continue;
        }
        if yaml_key(trimmed).is_some_and(|key| key == "backend") {
            flush_kubernetes_ingress_backend(&mut ingress_backends, &mut active_ingress_backend);
            active_ingress_backend = Some((
                KubernetesIngressBackend {
                    service_name: String::new(),
                    service_port: None,
                    host: active_ingress_host.clone(),
                    path: active_ingress_path.clone(),
                    path_type: active_ingress_path_type.clone(),
                    line,
                },
                indent,
            ));
            active_ingress_service_indent = None;
            continue;
        }
        if active_ingress_backend.is_some() && yaml_key(trimmed).is_some_and(|key| key == "service")
        {
            active_ingress_service_indent = Some(indent);
            continue;
        }
        if active_ingress_service_indent.is_some()
            && let Some((backend, _)) = active_ingress_backend.as_mut()
        {
            if let Some(value) = yaml_key_value(trimmed, "name") {
                if backend.service_name.is_empty() {
                    backend.service_name = value;
                    backend.line = line;
                } else if backend.service_port.is_none() {
                    backend.service_port = Some(value);
                }
                continue;
            }
            if let Some(value) = yaml_key_value(trimmed, "number") {
                backend.service_port = Some(value);
                continue;
            }
        }
        if yaml_key(trimmed).is_some_and(|key| key == "template") {
            active_template_indent = Some(indent);
            continue;
        }
        if active_template_indent.is_some()
            && yaml_key(trimmed).is_some_and(|key| key == "metadata")
        {
            active_template_metadata_indent = Some(indent);
            continue;
        }
        if yaml_key(trimmed).is_some_and(|key| key == "selector") {
            active_selector_indent = Some(indent);
            continue;
        }
        if yaml_key(trimmed).is_some_and(|key| key == "labels") {
            if active_template_metadata_indent.is_some() {
                active_label_map = Some((KubernetesLabelTarget::PodTemplate, indent));
            } else if metadata_indent.is_some_and(|metadata_indent| indent > metadata_indent) {
                active_label_map = Some((KubernetesLabelTarget::Metadata, indent));
            }
            continue;
        }
        if active_selector_indent.is_some()
            && yaml_key(trimmed).is_some_and(|key| key == "matchLabels")
        {
            active_label_map = Some((KubernetesLabelTarget::Selector, indent));
            continue;
        }
        if active_selector_indent.is_some()
            && let Some((key, value)) = yaml_key_pair(trimmed)
            && key != "matchLabels"
        {
            selector_labels.insert(key, value);
            continue;
        }

        if let Some((config_kind, ref_kind)) = kubernetes_ref_key(item_trimmed) {
            if let Some(value) = yaml_key_value(item_trimmed, &ref_kind)
                && let Some(name) = yaml_inline_mapping_value(&value, "name")
            {
                config_refs.push(KubernetesConfigRef {
                    config_kind,
                    ref_kind,
                    name,
                    line,
                });
            } else {
                active_ref = Some((config_kind, ref_kind, indent, line));
            }
            continue;
        }

        if let Some(containers_indent) = active_containers_indent
            && indent == containers_indent + 2
            && trimmed
                .strip_prefix("- ")
                .and_then(|value| yaml_key_value(value, "name"))
                .is_some()
        {
            container_count += 1;
        }

        if active_ports_indent.is_some() {
            if let Some(value) = trimmed.strip_prefix("- ") {
                flush_kubernetes_service_port(&mut service_ports, &mut active_service_port);
                let mut port = KubernetesServicePort {
                    name: None,
                    port: None,
                    target_port: None,
                    protocol: "TCP".to_string(),
                    line,
                };
                if let Some((key, value)) = yaml_key_pair(value) {
                    apply_kubernetes_service_port_field(&mut port, key, value);
                }
                active_service_port = Some((port, indent));
            } else if let Some((key, value)) = yaml_key_pair(trimmed)
                && let Some((port, _)) = active_service_port.as_mut()
            {
                apply_kubernetes_service_port_field(port, key, value);
            }
        }
    }

    flush_kubernetes_service_port(&mut service_ports, &mut active_service_port);
    flush_kubernetes_ingress_backend(&mut ingress_backends, &mut active_ingress_backend);

    let kind = kind?;
    let name = name?;
    if !has_api_version || !kubernetes_known_kind(&kind) {
        return None;
    }

    Some(KubernetesDocument {
        kind,
        name,
        namespace: namespace.unwrap_or_else(|| "default".to_string()),
        line: start_line,
        labels,
        pod_labels,
        selector_labels,
        config_refs,
        service_ports,
        ingress_backends,
        container_count,
    })
}

pub(crate) fn kubernetes_ref_key(trimmed: &str) -> Option<(String, String)> {
    let ref_kind = yaml_key(trimmed)?;
    let config_kind = match ref_kind.as_str() {
        "configMapRef" | "configMapKeyRef" => "configmap",
        "secretRef" | "secretKeyRef" => "secret",
        _ => return None,
    };
    Some((config_kind.to_string(), ref_kind))
}

pub(crate) fn yaml_inline_mapping_value(value: &str, expected_key: &str) -> Option<String> {
    let value = value
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    for part in value.split(',') {
        if let Some(found) = yaml_key_value(part.trim(), expected_key) {
            return Some(found);
        }
    }
    None
}

pub(crate) fn apply_kubernetes_service_port_field(
    port: &mut KubernetesServicePort,
    key: String,
    value: String,
) {
    match key.as_str() {
        "name" if !value.is_empty() => port.name = Some(value),
        "port" if !value.is_empty() => port.port = Some(value),
        "targetPort" if !value.is_empty() => port.target_port = Some(value),
        "protocol" if !value.is_empty() => port.protocol = value.to_ascii_uppercase(),
        _ => {}
    }
}

pub(crate) fn flush_kubernetes_service_port(
    ports: &mut Vec<KubernetesServicePort>,
    active_port: &mut Option<(KubernetesServicePort, usize)>,
) {
    let Some((port, _)) = active_port.take() else {
        return;
    };
    if port.port.is_some() || port.target_port.is_some() {
        ports.push(port);
    }
}

pub(crate) fn flush_kubernetes_ingress_backend(
    backends: &mut Vec<KubernetesIngressBackend>,
    active_backend: &mut Option<(KubernetesIngressBackend, usize)>,
) {
    let Some((backend, _)) = active_backend.take() else {
        return;
    };
    if !backend.service_name.is_empty() {
        backends.push(backend);
    }
}

pub(crate) fn kubernetes_known_kind(kind: &str) -> bool {
    kubernetes_config_kind(kind).is_some()
        || matches!(kind, "Ingress" | "Service")
        || kubernetes_workload_kind(kind)
}

pub(crate) fn kubernetes_config_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "ConfigMap" => Some("configmap"),
        "Secret" => Some("secret"),
        _ => None,
    }
}

pub(crate) fn kubernetes_workload_kind(kind: &str) -> bool {
    matches!(
        kind,
        "Deployment" | "StatefulSet" | "DaemonSet" | "Job" | "CronJob" | "Pod"
    )
}

pub(crate) fn kubernetes_resource_label(kind: &str, namespace: &str, name: &str) -> String {
    format!("k8s {kind}:{namespace}/{name}")
}

pub(crate) fn github_actions_workflow(label: &str, source: &str) -> GithubActionsWorkflow {
    let fallback_name = Path::new(label)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("workflow")
        .to_string();
    let mut workflow_name = None;
    let mut workflow_environment = Vec::new();
    let mut jobs = Vec::new();
    let mut in_jobs = false;
    let mut jobs_indent = 0usize;
    let mut active_workflow_section: Option<(String, usize)> = None;
    let mut active_job: Option<GithubActionsJob> = None;
    let mut active_step: Option<(GithubActionsStep, usize)> = None;
    let mut active_section: Option<(String, usize)> = None;

    for (index, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = yaml_indent(raw_line);

        if !in_jobs {
            if let Some((section, section_indent)) = active_workflow_section.as_ref() {
                if indent <= *section_indent {
                    active_workflow_section = None;
                } else if section == "env" {
                    if let Some(environment) =
                        ci_environment_assignment(trimmed, "workflow", index as u32 + 1)
                    {
                        workflow_environment.push(environment);
                    }
                    continue;
                }
            }
            if indent == 0
                && workflow_name.is_none()
                && let Some(value) = yaml_key_value(trimmed, "name")
            {
                workflow_name = Some(value);
            }
            if indent == 0 && yaml_key(trimmed).is_some_and(|key| key == "env") {
                active_workflow_section = Some(("env".to_string(), indent));
                continue;
            }
            if yaml_key(trimmed).is_some_and(|key| key == "jobs") {
                in_jobs = true;
                jobs_indent = indent;
                active_workflow_section = None;
            }
            continue;
        }

        if indent <= jobs_indent && yaml_key(trimmed).is_some() {
            flush_github_actions_step(&mut active_job, &mut active_step);
            flush_github_actions_job(&mut active_job, &mut jobs);
            break;
        }

        if let Some(job) = active_job.as_mut()
            && indent > jobs_indent + 2
        {
            job.end_line = index as u32 + 1;
        }

        if indent == jobs_indent + 2 {
            flush_github_actions_step(&mut active_job, &mut active_step);
            flush_github_actions_job(&mut active_job, &mut jobs);
            active_section = None;
            let Some(id) = yaml_key(trimmed) else {
                continue;
            };
            active_job = Some(GithubActionsJob {
                id,
                display_name: None,
                runs_on: None,
                needs: Vec::new(),
                environment: Vec::new(),
                steps: Vec::new(),
                line: index as u32 + 1,
                end_line: index as u32 + 1,
            });
            continue;
        }

        if indent == jobs_indent + 4 {
            flush_github_actions_step(&mut active_job, &mut active_step);
            let Some(job) = active_job.as_mut() else {
                continue;
            };
            active_section = None;
            if let Some(value) = yaml_key_value(trimmed, "name") {
                job.display_name = Some(value);
            } else if let Some(value) = yaml_key_value(trimmed, "runs-on") {
                job.runs_on = Some(value);
            } else if let Some(value) = yaml_key_value(trimmed, "needs") {
                job.needs.extend(github_actions_needs_values(&value));
            } else if yaml_key(trimmed).is_some_and(|key| key == "needs") {
                active_section = Some(("needs".to_string(), indent));
            } else if yaml_key(trimmed).is_some_and(|key| key == "env") {
                active_section = Some(("env".to_string(), indent));
            } else if yaml_key(trimmed).is_some_and(|key| key == "steps") {
                active_section = Some(("steps".to_string(), indent));
            }
            continue;
        }

        let Some((section, section_indent)) = active_section.as_ref() else {
            continue;
        };
        if indent <= *section_indent {
            flush_github_actions_step(&mut active_job, &mut active_step);
            active_section = None;
            continue;
        }

        match section.as_str() {
            "needs" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    let dependency = yaml_clean_scalar(value);
                    if !dependency.is_empty()
                        && let Some(job) = active_job.as_mut()
                    {
                        job.needs.push(dependency);
                    }
                }
            }
            "env" => {
                if let Some(environment) =
                    ci_environment_assignment(trimmed, "job", index as u32 + 1)
                    && let Some(job) = active_job.as_mut()
                {
                    job.environment.push(environment);
                }
            }
            "steps" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    flush_github_actions_step(&mut active_job, &mut active_step);
                    let mut step = GithubActionsStep {
                        name: None,
                        uses: None,
                        run: None,
                        line: index as u32 + 1,
                    };
                    apply_github_actions_step_field(&mut step, value);
                    active_step = Some((step, indent));
                } else if let Some((step, step_indent)) = active_step.as_mut()
                    && indent > *step_indent
                {
                    apply_github_actions_step_field(step, trimmed);
                }
            }
            _ => {}
        }
    }

    flush_github_actions_step(&mut active_job, &mut active_step);
    flush_github_actions_job(&mut active_job, &mut jobs);
    GithubActionsWorkflow {
        name: workflow_name.unwrap_or(fallback_name),
        environment: workflow_environment,
        jobs,
    }
}

pub(crate) fn flush_github_actions_job(
    active_job: &mut Option<GithubActionsJob>,
    jobs: &mut Vec<GithubActionsJob>,
) {
    let Some(mut job) = active_job.take() else {
        return;
    };
    job.needs.sort();
    job.needs.dedup();
    jobs.push(job);
}

pub(crate) fn flush_github_actions_step(
    active_job: &mut Option<GithubActionsJob>,
    active_step: &mut Option<(GithubActionsStep, usize)>,
) {
    let Some((step, _)) = active_step.take() else {
        return;
    };
    if (step.uses.is_some() || step.run.is_some())
        && let Some(job) = active_job.as_mut()
    {
        job.steps.push(step);
    }
}

pub(crate) fn apply_github_actions_step_field(step: &mut GithubActionsStep, field: &str) {
    if let Some(value) = yaml_key_value(field, "name") {
        step.name = Some(value);
    } else if let Some(value) = yaml_key_value(field, "uses") {
        step.uses = Some(value);
    } else if let Some(value) = yaml_key_value(field, "run") {
        step.run = Some(value);
    }
}

pub(crate) fn github_actions_needs_values(value: &str) -> Vec<String> {
    let inline = yaml_inline_list_values(value);
    if !inline.is_empty() {
        return inline;
    }
    let value = yaml_clean_scalar(value);
    (!value.is_empty()).then_some(value).into_iter().collect()
}

pub(crate) fn github_actions_local_action_path(action: &str) -> Option<String> {
    let action = action.trim().trim_matches('"').trim_matches('\'');
    if !action.starts_with("./") {
        return None;
    }
    normalize_relative_path(Path::new(action))
}

pub(crate) fn github_actions_remote_action(action: &str) -> (String, Option<String>) {
    let action = action.trim();
    let (name, version) = action
        .rsplit_once('@')
        .map(|(name, version)| (name.trim(), Some(version.trim().to_string())))
        .unwrap_or((action, None));
    let name = name.trim_matches('"').trim_matches('\'').to_string();
    let version = version.filter(|value| !value.is_empty());
    (name, version)
}

pub(crate) fn is_github_actions_workflow_path(label: &str, path: &Path) -> bool {
    let name = path.file_name().and_then(|name| name.to_str());
    matches!(name, Some(name) if name.ends_with(".yml") || name.ends_with(".yaml"))
        && label.starts_with(".github/workflows/")
}

pub(crate) fn gitlab_ci_jobs(source: &str) -> Vec<GitlabCiJob> {
    let mut jobs = Vec::new();
    let mut global_variables = Vec::new();
    let mut active_job: Option<GitlabCiJob> = None;
    let mut active_global_section: Option<(String, usize)> = None;
    let mut active_section: Option<(String, usize)> = None;

    for (index, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = yaml_indent(raw_line);
        if let Some((section, section_indent)) = active_global_section.as_ref() {
            if indent <= *section_indent {
                active_global_section = None;
            } else if section == "variables" {
                if let Some(variable) =
                    ci_environment_assignment(trimmed, "pipeline", index as u32 + 1)
                {
                    global_variables.push(variable);
                }
                continue;
            }
        }
        if indent == 0 {
            flush_gitlab_ci_job(&mut active_job, &mut jobs);
            active_section = None;
            let Some(name) = yaml_key(trimmed) else {
                continue;
            };
            if name == "variables" {
                active_global_section = Some(("variables".to_string(), indent));
                continue;
            }
            if gitlab_ci_reserved_key(&name) || name.starts_with('.') {
                continue;
            }
            active_job = Some(GitlabCiJob {
                name,
                stage: None,
                image: None,
                extends: Vec::new(),
                needs: Vec::new(),
                dependencies: Vec::new(),
                variables: global_variables.clone(),
                scripts: Vec::new(),
                line: index as u32 + 1,
            });
            continue;
        }

        let Some(job) = active_job.as_mut() else {
            continue;
        };

        if indent == 2 {
            active_section = None;
            if let Some(value) = yaml_key_value(trimmed, "stage") {
                job.stage = Some(value);
            } else if let Some(value) = yaml_key_value(trimmed, "image") {
                job.image = Some(value);
            } else if let Some(value) = yaml_key_value(trimmed, "extends") {
                job.extends.extend(gitlab_ci_name_values(&value));
            } else if yaml_key(trimmed).is_some_and(|key| key == "extends") {
                active_section = Some(("extends".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "needs") {
                job.needs.extend(gitlab_ci_name_values(&value));
            } else if yaml_key(trimmed).is_some_and(|key| key == "needs") {
                active_section = Some(("needs".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "dependencies") {
                job.dependencies.extend(gitlab_ci_name_values(&value));
            } else if yaml_key(trimmed).is_some_and(|key| key == "dependencies") {
                active_section = Some(("dependencies".to_string(), indent));
            } else if yaml_key(trimmed).is_some_and(|key| key == "variables") {
                active_section = Some(("variables".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "script") {
                push_gitlab_ci_script(job, "script", value, index as u32 + 1);
            } else if yaml_key(trimmed).is_some_and(|key| key == "script") {
                active_section = Some(("script".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "before_script") {
                push_gitlab_ci_script(job, "before_script", value, index as u32 + 1);
            } else if yaml_key(trimmed).is_some_and(|key| key == "before_script") {
                active_section = Some(("before_script".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "after_script") {
                push_gitlab_ci_script(job, "after_script", value, index as u32 + 1);
            } else if yaml_key(trimmed).is_some_and(|key| key == "after_script") {
                active_section = Some(("after_script".to_string(), indent));
            }
            continue;
        }

        let Some((section, section_indent)) = active_section.as_ref() else {
            continue;
        };
        if indent <= *section_indent {
            active_section = None;
            continue;
        }

        match section.as_str() {
            "extends" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    job.extends.extend(gitlab_ci_name_values(value));
                }
            }
            "needs" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    job.needs.extend(gitlab_ci_need_values(value));
                } else if let Some(value) = yaml_key_value(trimmed, "job") {
                    job.needs.push(value);
                }
            }
            "dependencies" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    job.dependencies.extend(gitlab_ci_name_values(value));
                }
            }
            "variables" => {
                if let Some(variable) = ci_environment_assignment(trimmed, "job", index as u32 + 1)
                {
                    job.variables.push(variable);
                }
            }
            "script" | "before_script" | "after_script" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    push_gitlab_ci_script(job, section, yaml_clean_scalar(value), index as u32 + 1);
                }
            }
            _ => {}
        }
    }

    flush_gitlab_ci_job(&mut active_job, &mut jobs);
    jobs
}

pub(crate) fn flush_gitlab_ci_job(
    active_job: &mut Option<GitlabCiJob>,
    jobs: &mut Vec<GitlabCiJob>,
) {
    let Some(mut job) = active_job.take() else {
        return;
    };
    job.extends.sort();
    job.extends.dedup();
    job.needs.sort();
    job.needs.dedup();
    job.dependencies.sort();
    job.dependencies.dedup();
    if !job.scripts.is_empty() || !job.needs.is_empty() || !job.dependencies.is_empty() {
        jobs.push(job);
    }
}

pub(crate) fn push_gitlab_ci_script(
    job: &mut GitlabCiJob,
    script_kind: &str,
    command: String,
    line: u32,
) {
    let inline = yaml_inline_list_values(&command);
    if !inline.is_empty() {
        for command in inline {
            if !command.is_empty() {
                job.scripts.push(GitlabCiScript {
                    command,
                    script_kind: script_kind.to_string(),
                    ordinal: job.scripts.len() + 1,
                    line,
                });
            }
        }
        return;
    }
    let command = yaml_clean_scalar(&command);
    if command.is_empty() || command == "|" || command == ">" {
        return;
    }
    job.scripts.push(GitlabCiScript {
        command,
        script_kind: script_kind.to_string(),
        ordinal: job.scripts.len() + 1,
        line,
    });
}

pub(crate) fn gitlab_ci_name_values(value: &str) -> Vec<String> {
    let inline = yaml_inline_list_values(value);
    if !inline.is_empty() {
        return inline;
    }
    let value = yaml_clean_scalar(value);
    (!value.is_empty()).then_some(value).into_iter().collect()
}

pub(crate) fn gitlab_ci_need_values(value: &str) -> Vec<String> {
    if let Some(job) = yaml_key_value(value.trim(), "job") {
        return vec![job];
    }
    gitlab_ci_name_values(value)
}

pub(crate) fn ci_environment_assignment(
    trimmed: &str,
    scope: &str,
    line: u32,
) -> Option<CiEnvironment> {
    let name = yaml_key(trimmed)?;
    let value = yaml_key_value(trimmed, &name);
    let value_present = value.is_some();
    let value_kind = value
        .as_deref()
        .map(ci_environment_value_kind)
        .unwrap_or("runner")
        .to_string();
    Some(CiEnvironment {
        name,
        value_present,
        value_kind,
        scope: scope.to_string(),
        line,
    })
}

pub(crate) fn ci_environment_value_kind(value: &str) -> &'static str {
    let value = value.trim();
    if value.contains("secrets.") || value.contains("vault.") {
        "secret_reference"
    } else if value.contains("${{") || value.contains("$[") {
        "expression"
    } else if value.starts_with('$') || value.contains("${") || value.contains('%') {
        "variable_reference"
    } else {
        "literal"
    }
}

pub(crate) fn gitlab_ci_reserved_key(key: &str) -> bool {
    matches!(
        key,
        "after_script"
            | "before_script"
            | "cache"
            | "default"
            | "image"
            | "include"
            | "services"
            | "stages"
            | "variables"
            | "workflow"
    )
}

pub(crate) fn is_gitlab_ci_path(label: &str, path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == ".gitlab-ci.yml" || name == ".gitlab-ci.yaml")
        || label.starts_with(".gitlab/ci/")
            && matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some(name) if name.ends_with(".yml") || name.ends_with(".yaml")
            )
}

pub(crate) fn kubernetes_config_ref_label(
    config_kind: &str,
    namespace: &str,
    name: &str,
) -> String {
    format!("k8s config ref:{config_kind} {namespace}/{name}")
}

pub(crate) fn kubernetes_service_ref_label(namespace: &str, name: &str) -> String {
    format!("k8s service ref:{namespace}/{name}")
}

pub(crate) fn kubernetes_service_port_label(
    document: &KubernetesDocument,
    port: &KubernetesServicePort,
) -> String {
    let port_value = port.port.as_deref().unwrap_or("unknown");
    match port.target_port.as_deref() {
        Some(target) => format!(
            "k8s service port:{}/{}:{}->{}/{}",
            document.namespace, document.name, port_value, target, port.protocol
        ),
        None => format!(
            "k8s service port:{}/{}:{}/{}",
            document.namespace, document.name, port_value, port.protocol
        ),
    }
}

pub(crate) fn kubernetes_workload_match_labels(
    document: &KubernetesDocument,
) -> &BTreeMap<String, String> {
    if !document.pod_labels.is_empty() {
        &document.pod_labels
    } else {
        &document.labels
    }
}

pub(crate) fn kubernetes_selector_matches(
    selector: &BTreeMap<String, String>,
    labels: &BTreeMap<String, String>,
) -> bool {
    !selector.is_empty()
        && selector
            .iter()
            .all(|(key, value)| labels.get(key).is_some_and(|label| label == value))
}

pub(crate) fn kubernetes_label_selector_string(labels: &BTreeMap<String, String>) -> String {
    labels
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn compose_services(source: &str) -> Vec<ComposeService> {
    let mut services = Vec::new();
    let mut in_services = false;
    let mut services_indent = 0usize;
    let mut active_service: Option<ComposeService> = None;
    let mut active_section: Option<(String, usize)> = None;
    let mut active_port: Option<ComposePort> = None;
    let mut active_volume: Option<ComposeVolume> = None;

    for (index, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = yaml_indent(raw_line);

        if !in_services {
            if yaml_key(trimmed).is_some_and(|key| key == "services") {
                in_services = true;
                services_indent = indent;
            }
            continue;
        }

        if indent <= services_indent && yaml_key(trimmed).is_some() {
            break;
        }

        if indent == services_indent + 2 {
            if let Some(mut service) = active_service.take() {
                flush_compose_service_port(&mut service, &mut active_port);
                flush_compose_service_volume(&mut service, &mut active_volume);
                services.push(service);
            }
            active_section = None;
            let Some(name) = yaml_key(trimmed).filter(|name| !compose_service_reserved_key(name))
            else {
                continue;
            };
            active_service = Some(ComposeService {
                name,
                command: None,
                command_kind: None,
                build_context: None,
                dockerfile: None,
                depends_on: Vec::new(),
                environment: Vec::new(),
                env_files: Vec::new(),
                ports: Vec::new(),
                volumes: Vec::new(),
                line: index as u32 + 1,
            });
            continue;
        }

        let Some(service) = active_service.as_mut() else {
            continue;
        };

        if indent == services_indent + 4 {
            if active_section
                .as_ref()
                .is_some_and(|(section, _)| section == "ports")
            {
                flush_compose_service_port(service, &mut active_port);
            }
            if active_section
                .as_ref()
                .is_some_and(|(section, _)| section == "volumes")
            {
                flush_compose_service_volume(service, &mut active_volume);
            }
            active_section = None;
            if let Some(value) = yaml_key_value(trimmed, "command") {
                service.command = compose_command_string(&value);
                service.command_kind = Some("command".to_string());
            } else if let Some(value) = yaml_key_value(trimmed, "entrypoint") {
                service.command = compose_command_string(&value);
                service.command_kind = Some("entrypoint".to_string());
            } else if let Some(value) = yaml_key_value(trimmed, "build") {
                service.build_context = Some(value);
            } else if yaml_key(trimmed).is_some_and(|key| key == "build") {
                active_section = Some(("build".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "depends_on") {
                service.depends_on.extend(compose_inline_depends_on(&value));
            } else if yaml_key(trimmed).is_some_and(|key| key == "depends_on") {
                active_section = Some(("depends_on".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "environment") {
                service
                    .environment
                    .extend(compose_inline_environment(&value, index as u32 + 1));
            } else if yaml_key(trimmed).is_some_and(|key| key == "environment") {
                active_section = Some(("environment".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "env_file") {
                service
                    .env_files
                    .extend(compose_env_file_values(&value, index as u32 + 1));
            } else if yaml_key(trimmed).is_some_and(|key| key == "env_file") {
                active_section = Some(("env_file".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "ports") {
                service
                    .ports
                    .extend(compose_inline_ports(&value, index as u32 + 1));
            } else if yaml_key(trimmed).is_some_and(|key| key == "ports") {
                active_section = Some(("ports".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "volumes") {
                service
                    .volumes
                    .extend(compose_inline_volumes(&value, index as u32 + 1));
            } else if yaml_key(trimmed).is_some_and(|key| key == "volumes") {
                active_section = Some(("volumes".to_string(), indent));
            }
            continue;
        }

        let Some((section, section_indent)) = active_section.as_ref() else {
            continue;
        };
        if indent <= *section_indent {
            active_section = None;
            continue;
        }

        match section.as_str() {
            "build" => {
                if let Some(value) = yaml_key_value(trimmed, "context") {
                    service.build_context = Some(value);
                } else if let Some(value) = yaml_key_value(trimmed, "dockerfile") {
                    service.dockerfile = Some(value);
                }
            }
            "depends_on" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    let dependency = yaml_clean_scalar(value);
                    if !dependency.is_empty() {
                        service.depends_on.push(dependency);
                    }
                } else if let Some(name) = yaml_key(trimmed)
                    && !compose_depends_on_option_key(&name)
                {
                    service.depends_on.push(name);
                }
            }
            "environment" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    if let Some(environment) =
                        compose_environment_assignment(value, index as u32 + 1)
                    {
                        service.environment.push(environment);
                    }
                } else if let Some(name) = yaml_key(trimmed) {
                    let value_present = yaml_key_value(trimmed, &name).is_some();
                    service.environment.push(ComposeEnvironment {
                        name,
                        value_present,
                        line: index as u32 + 1,
                    });
                }
            }
            "env_file" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    let path = yaml_clean_scalar(value);
                    if !path.is_empty() {
                        service.env_files.push(ComposeEnvFile {
                            path,
                            line: index as u32 + 1,
                        });
                    }
                }
            }
            "ports" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    flush_compose_service_port(service, &mut active_port);
                    if let Some(port) = compose_short_port(value, index as u32 + 1) {
                        service.ports.push(port);
                    } else if let Some((key, value)) = yaml_key_pair(value) {
                        active_port = Some(compose_long_port(key, value, index as u32 + 1));
                    }
                } else if let Some((key, value)) = yaml_key_pair(trimmed)
                    && let Some(port) = active_port.as_mut()
                {
                    apply_compose_port_field(port, key, value);
                }
            }
            "volumes" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    flush_compose_service_volume(service, &mut active_volume);
                    if let Some(volume) = compose_short_volume(value, index as u32 + 1) {
                        service.volumes.push(volume);
                    } else if let Some((key, value)) = yaml_key_pair(value) {
                        active_volume = Some(compose_long_volume(key, value, index as u32 + 1));
                    }
                } else if let Some((key, value)) = yaml_key_pair(trimmed)
                    && let Some(volume) = active_volume.as_mut()
                {
                    apply_compose_volume_field(volume, key, value);
                }
            }
            _ => {}
        }
    }

    if let Some(mut service) = active_service {
        flush_compose_service_port(&mut service, &mut active_port);
        flush_compose_service_volume(&mut service, &mut active_volume);
        services.push(service);
    }
    dedupe_compose_service_dependencies(&mut services);
    services
}

pub(crate) fn compose_service_reserved_key(name: &str) -> bool {
    matches!(
        name,
        "build"
            | "command"
            | "depends_on"
            | "entrypoint"
            | "image"
            | "networks"
            | "volumes"
            | "environment"
            | "env_file"
            | "ports"
    )
}

pub(crate) fn compose_depends_on_option_key(name: &str) -> bool {
    matches!(
        name,
        "condition" | "restart" | "required" | "service_healthy" | "service_started"
    )
}

pub(crate) fn compose_command_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with('[') {
        dockerfile_exec_form_command(value)
    } else {
        Some(value.to_string())
    }
}

pub(crate) fn compose_inline_depends_on(value: &str) -> Vec<String> {
    let value = value.trim();
    if value.starts_with('[') {
        yaml_inline_list_values(value)
    } else if value.is_empty() {
        Vec::new()
    } else {
        vec![value.to_string()]
    }
}

pub(crate) fn compose_inline_environment(value: &str, line: u32) -> Vec<ComposeEnvironment> {
    yaml_inline_list_values(value)
        .into_iter()
        .filter_map(|value| compose_environment_assignment(&value, line))
        .collect()
}

pub(crate) fn compose_environment_assignment(value: &str, line: u32) -> Option<ComposeEnvironment> {
    let value = yaml_clean_scalar(value);
    let (name, value_present) = value
        .split_once('=')
        .map(|(name, _)| (name.trim(), true))
        .unwrap_or((value.trim(), false));
    (!name.is_empty()).then(|| ComposeEnvironment {
        name: name.to_string(),
        value_present,
        line,
    })
}

pub(crate) fn compose_env_file_values(value: &str, line: u32) -> Vec<ComposeEnvFile> {
    let values = if value.trim().starts_with('[') {
        yaml_inline_list_values(value)
    } else {
        vec![yaml_clean_scalar(value)]
    };
    values
        .into_iter()
        .filter(|path| !path.is_empty())
        .map(|path| ComposeEnvFile { path, line })
        .collect()
}

pub(crate) fn compose_inline_ports(value: &str, line: u32) -> Vec<ComposePort> {
    yaml_inline_list_values(value)
        .into_iter()
        .filter_map(|value| compose_short_port(&value, line))
        .collect()
}

pub(crate) fn compose_short_port(value: &str, line: u32) -> Option<ComposePort> {
    let raw = yaml_clean_scalar(value);
    if raw.is_empty() || yaml_key_pair(&raw).is_some() {
        return None;
    }
    let (port_spec, protocol) = compose_port_protocol(&raw);
    let parts: Vec<_> = port_spec.split(':').map(str::trim).collect();
    let (host_ip, published, target) = match parts.as_slice() {
        [target] if !target.is_empty() => (None, None, Some((*target).to_string())),
        [published, target] if !published.is_empty() && !target.is_empty() => (
            None,
            Some((*published).to_string()),
            Some((*target).to_string()),
        ),
        [host_ip, published, target]
            if !host_ip.is_empty() && !published.is_empty() && !target.is_empty() =>
        {
            (
                Some((*host_ip).to_string()),
                Some((*published).to_string()),
                Some((*target).to_string()),
            )
        }
        _ => return None,
    };
    Some(ComposePort {
        published,
        target,
        protocol,
        host_ip,
        raw: Some(raw),
        line,
    })
}

pub(crate) fn compose_long_port(key: String, value: String, line: u32) -> ComposePort {
    let mut port = ComposePort {
        published: None,
        target: None,
        protocol: "tcp".to_string(),
        host_ip: None,
        raw: None,
        line,
    };
    apply_compose_port_field(&mut port, key, value);
    port
}

pub(crate) fn apply_compose_port_field(port: &mut ComposePort, key: String, value: String) {
    match key.as_str() {
        "published" => port.published = Some(value),
        "target" => port.target = Some(value),
        "protocol" if !value.is_empty() => port.protocol = value,
        "host_ip" if !value.is_empty() => port.host_ip = Some(value),
        _ => {}
    }
}

pub(crate) fn flush_compose_service_port(
    service: &mut ComposeService,
    active_port: &mut Option<ComposePort>,
) {
    let Some(port) = active_port.take() else {
        return;
    };
    if port.published.is_some() || port.target.is_some() {
        service.ports.push(port);
    }
}

pub(crate) fn compose_port_protocol(raw: &str) -> (String, String) {
    if let Some((port, protocol)) = raw.rsplit_once('/')
        && !port.is_empty()
        && !protocol.is_empty()
        && protocol
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        return (port.to_string(), protocol.to_ascii_lowercase());
    }
    (raw.to_string(), "tcp".to_string())
}

pub(crate) fn compose_port_label(port: &ComposePort) -> String {
    let protocol = port.protocol.as_str();
    match (
        port.host_ip.as_deref(),
        port.published.as_deref(),
        port.target.as_deref(),
    ) {
        (Some(host_ip), Some(published), Some(target)) => {
            format!("compose port:{host_ip}:{published}->{target}/{protocol}")
        }
        (_, Some(published), Some(target)) => {
            format!("compose port:{published}->{target}/{protocol}")
        }
        (_, Some(published), None) => format!("compose port:{published}/{protocol}"),
        (_, None, Some(target)) => format!("compose port:{target}/{protocol}"),
        _ => "compose port:unknown".to_string(),
    }
}

pub(crate) fn compose_inline_volumes(value: &str, line: u32) -> Vec<ComposeVolume> {
    yaml_inline_list_values(value)
        .into_iter()
        .filter_map(|value| compose_short_volume(&value, line))
        .collect()
}

pub(crate) fn compose_short_volume(value: &str, line: u32) -> Option<ComposeVolume> {
    let raw = yaml_clean_scalar(value);
    if raw.is_empty() || yaml_key_pair(&raw).is_some() {
        return None;
    }
    let parts: Vec<_> = raw.split(':').map(str::trim).collect();
    let (source, target, read_only) = match parts.as_slice() {
        [target] if !target.is_empty() => (None, Some((*target).to_string()), false),
        [source, target] if !source.is_empty() && !target.is_empty() => (
            Some((*source).to_string()),
            Some((*target).to_string()),
            false,
        ),
        [source, target, mode] if !source.is_empty() && !target.is_empty() => (
            Some((*source).to_string()),
            Some((*target).to_string()),
            compose_volume_mode_read_only(mode),
        ),
        _ => return None,
    };
    Some(ComposeVolume {
        source,
        target,
        kind: "volume".to_string(),
        read_only,
        raw: Some(raw),
        line,
    })
}

pub(crate) fn compose_long_volume(key: String, value: String, line: u32) -> ComposeVolume {
    let mut volume = ComposeVolume {
        source: None,
        target: None,
        kind: "volume".to_string(),
        read_only: false,
        raw: None,
        line,
    };
    apply_compose_volume_field(&mut volume, key, value);
    volume
}

pub(crate) fn apply_compose_volume_field(volume: &mut ComposeVolume, key: String, value: String) {
    match key.as_str() {
        "type" if !value.is_empty() => volume.kind = value,
        "source" if !value.is_empty() => volume.source = Some(value),
        "target" if !value.is_empty() => volume.target = Some(value),
        "read_only" | "readonly" => volume.read_only = yaml_truthy(&value),
        _ => {}
    }
}

pub(crate) fn flush_compose_service_volume(
    service: &mut ComposeService,
    active_volume: &mut Option<ComposeVolume>,
) {
    let Some(volume) = active_volume.take() else {
        return;
    };
    if volume.source.is_some() || volume.target.is_some() {
        service.volumes.push(volume);
    }
}

pub(crate) fn compose_volume_mode_read_only(mode: &str) -> bool {
    mode.split(',').any(|value| {
        let value = value.trim();
        matches!(value, "ro" | "readonly" | "read_only")
    })
}

pub(crate) fn yaml_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "yes" | "1" | "on"
    )
}

pub(crate) fn compose_volume_local_source_path(
    compose_label: &str,
    source: &str,
) -> Option<String> {
    let source = source.trim();
    if source.is_empty()
        || source.starts_with('$')
        || source.contains("://")
        || Path::new(source).is_absolute()
    {
        return None;
    }
    if !source.starts_with('.')
        && !source.contains('/')
        && !source.contains('\\')
        && Path::new(source).extension().is_none()
    {
        return None;
    }
    normalize_manifest_relative_path(compose_label, source)
}

pub(crate) fn compose_volume_label(volume: &ComposeVolume) -> String {
    match (volume.source.as_deref(), volume.target.as_deref()) {
        (Some(source), Some(target)) => format!("compose volume:{source}->{target}"),
        (Some(source), None) => format!("compose volume:{source}"),
        (None, Some(target)) => format!("compose volume:{target}"),
        _ => "compose volume:unknown".to_string(),
    }
}

pub(crate) fn yaml_inline_list_values(value: &str) -> Vec<String> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return Vec::new();
    }
    let body = value.trim_start_matches('[').trim_end_matches(']');
    let quoted = quoted_strings(body);
    if !quoted.is_empty() {
        return quoted
            .into_iter()
            .map(|value| yaml_clean_scalar(&value))
            .filter(|value| !value.is_empty())
            .collect();
    }
    body.split(',')
        .map(yaml_clean_scalar)
        .filter(|value| !value.is_empty())
        .collect()
}

pub(crate) fn compose_service_dockerfile_path(
    compose_label: &str,
    service: &ComposeService,
) -> Option<String> {
    compose_service_dockerfile_target(service)
        .and_then(|target| normalize_manifest_relative_path(compose_label, &target))
}

pub(crate) fn compose_service_dockerfile_target(service: &ComposeService) -> Option<String> {
    if service.build_context.is_none() && service.dockerfile.is_none() {
        return None;
    }
    let dockerfile = service.dockerfile.as_deref().unwrap_or("Dockerfile").trim();
    let context = service.build_context.as_deref().unwrap_or(".").trim();
    if context.contains("://") || Path::new(context).is_absolute() || dockerfile.is_empty() {
        return None;
    }
    Some(if context == "." {
        dockerfile.to_string()
    } else {
        join_path(Some(context), dockerfile)
    })
}

pub(crate) fn dedupe_compose_service_dependencies(services: &mut [ComposeService]) {
    for service in services {
        service.depends_on.sort();
        service.depends_on.dedup();
        service
            .depends_on
            .retain(|dependency| dependency != &service.name);
    }
}

pub(crate) fn dockerfile_entrypoints(source: &str) -> Vec<DockerfileEntrypoint> {
    dockerfile_logical_lines(source)
        .into_iter()
        .filter_map(|(line, text)| dockerfile_entrypoint_line(&text, line))
        .collect()
}

pub(crate) fn dockerfile_logical_lines(source: &str) -> Vec<(u32, String)> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut start_line = 1;

    for (index, raw_line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let line = raw_line.trim();
        if current.is_empty() {
            start_line = line_number;
        }
        let continued = line.ends_with('\\');
        let part = line.trim_end_matches('\\').trim_end();
        if !current.is_empty() && !part.is_empty() {
            current.push(' ');
        }
        current.push_str(part);
        if !continued {
            lines.push((start_line, current.trim().to_string()));
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        lines.push((start_line, current.trim().to_string()));
    }

    lines
}

pub(crate) fn dockerfile_entrypoint_line(
    line: &str,
    line_number: u32,
) -> Option<DockerfileEntrypoint> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let instruction = parts.next()?.to_ascii_uppercase();
    if !matches!(instruction.as_str(), "ENTRYPOINT" | "CMD") {
        return None;
    }
    let body = parts.next()?.trim();
    if body.is_empty() {
        return None;
    }
    let command = if body.starts_with('[') {
        dockerfile_exec_form_command(body)
    } else {
        Some(body.to_string())
    }?;
    (!command.is_empty()).then_some(DockerfileEntrypoint {
        instruction,
        command,
        line: line_number,
    })
}

pub(crate) fn dockerfile_exec_form_command(body: &str) -> Option<String> {
    let values = quoted_strings(body);
    (!values.is_empty()).then(|| values.join(" "))
}

pub(crate) fn quoted_strings(value: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut chars = value.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '"' && character != '\'' {
            continue;
        }
        let quote = character;
        let mut item = String::new();
        let mut escaped = false;
        for next in chars.by_ref() {
            if escaped {
                item.push(next);
                escaped = false;
                continue;
            }
            if next == '\\' {
                escaped = true;
                continue;
            }
            if next == quote {
                break;
            }
            item.push(next);
        }
        values.push(item);
    }
    values
}

pub(crate) fn makefile_targets(source: &str) -> Vec<MakefileTarget> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut targets = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        let line_number = index as u32 + 1;
        let Some((target_names, inline_command)) = makefile_target_line(line) else {
            index += 1;
            continue;
        };

        let command = inline_command.or_else(|| {
            lines[index + 1..]
                .iter()
                .take_while(|line| makefile_recipe_or_blank_line(line))
                .find_map(makefile_recipe_command)
        });

        for name in target_names {
            targets.push(MakefileTarget {
                name,
                command: command.clone(),
                line: line_number,
            });
        }
        index += 1;
    }

    targets
}

pub(crate) fn makefile_target_line(line: &str) -> Option<(Vec<String>, Option<String>)> {
    if line.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }
    let candidate = line.split('#').next().unwrap_or("").trim_end();
    if candidate.is_empty() {
        return None;
    }
    let colon = candidate.find(':')?;
    if candidate[..colon].contains('=') {
        return None;
    }
    let before = candidate[..colon].trim();
    if before.is_empty() || before.starts_with('.') {
        return None;
    }

    let names = before
        .split_whitespace()
        .filter(|name| is_makefile_task_target(name))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if names.is_empty() {
        return None;
    }

    let inline_command = candidate[colon + 1..]
        .split_once(';')
        .map(|(_, command)| command.trim().to_string())
        .filter(|command| !command.is_empty());
    Some((names, inline_command))
}

pub(crate) fn is_makefile_task_target(name: &str) -> bool {
    let Some(first) = name.chars().next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() || name.contains('/') || name.contains('%') {
        return false;
    }
    name.chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
}

pub(crate) fn makefile_recipe_or_blank_line(line: &&str) -> bool {
    line.trim().is_empty() || line.starts_with('\t') || line.starts_with("        ")
}

pub(crate) fn makefile_recipe_command(line: &&str) -> Option<String> {
    if !line.starts_with('\t') && !line.starts_with("        ") {
        return None;
    }
    let command = line
        .trim_start()
        .trim_start_matches(['@', '-', '+'])
        .trim()
        .to_string();
    (!command.is_empty()).then_some(command)
}

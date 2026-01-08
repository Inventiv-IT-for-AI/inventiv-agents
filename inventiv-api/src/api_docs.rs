use crate::settings;
use crate::workbench;
use inventiv_common::{Instance, InstanceStatus, InstanceType, LlmModel, Region, Zone};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::handlers::instances::list_instances,
        crate::handlers::deployments::create_deployment,
        crate::handlers::instances::terminate_instance,
        // Models
        crate::handlers::models::list_models,
        crate::handlers::models::get_model,
        crate::handlers::models::create_model,
        crate::handlers::models::update_model,
        crate::handlers::models::delete_model,
        // Settings
        settings::list_regions,
        settings::update_region,
        settings::list_zones,
        settings::update_zone,
        settings::list_instance_types,
        settings::update_instance_type
        ,
        // Workbench (persistence)
        workbench::create_workbench_run,
        workbench::list_workbench_runs,
        workbench::get_workbench_run,
        workbench::append_workbench_message,
        workbench::complete_workbench_run
    ),
    components(
        schemas(
            crate::handlers::deployments::DeploymentRequest,
            crate::handlers::deployments::DeploymentResponse,
            crate::handlers::models::CreateModelRequest,
            crate::handlers::models::UpdateModelRequest,
            crate::handlers::models::ListModelsParams,
            Instance,
            InstanceStatus,
            LlmModel,
            // Settings
            Region,
            Zone,
            InstanceType,
            settings::UpdateRegionRequest,
            settings::UpdateZoneRequest,
            settings::UpdateInstanceTypeRequest
            ,
            // Workbench
            workbench::WorkbenchRunRow,
            workbench::WorkbenchMessageRow,
            workbench::CreateWorkbenchRunRequest,
            workbench::CreateWorkbenchRunResponse,
            workbench::AppendWorkbenchMessageRequest,
            workbench::AppendWorkbenchMessageResponse,
            workbench::CompleteWorkbenchRunRequest,
            workbench::WorkbenchRunWithMessages,
            workbench::ListWorkbenchRunsQuery
        )
    ),
    tags(
        (name = "inventiv-backend", description = "Inventiv Infrastructure API")
    )
)]
pub struct ApiDoc;

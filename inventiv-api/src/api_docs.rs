use utoipa::OpenApi;
use inventiv_common::{Instance, InstanceStatus, Region, Zone, InstanceType, LlmModel};
use crate::settings;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::list_instances,
        crate::create_deployment,
        crate::terminate_instance,
        // Models
        crate::list_models,
        crate::get_model,
        crate::create_model,
        crate::update_model,
        crate::delete_model,
        // Settings
        settings::list_regions,
        settings::update_region,
        settings::list_zones,
        settings::update_zone,
        settings::list_instance_types,
        settings::update_instance_type
    ),
    components(
        schemas(
            crate::DeploymentRequest, 
            crate::DeploymentResponse,
            crate::CreateModelRequest,
            crate::UpdateModelRequest,
            crate::ListModelsParams,
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
        )
    ),
    tags(
        (name = "inventiv-backend", description = "Inventiv Infrastructure API")
    )
)]
pub struct ApiDoc;

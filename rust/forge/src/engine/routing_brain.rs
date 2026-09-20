//! Port of forge-routing-brain.ts.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeRoutingBrain {
    Reducer,
    Engine,
}

pub fn parse_forge_routing_brain(raw: Option<&str>) -> ForgeRoutingBrain {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("reducer") => ForgeRoutingBrain::Reducer,
        _ => ForgeRoutingBrain::Engine,
    }
}

pub fn detect_forge_dual_write(
    story_id: &str,
    reducer_touched: bool,
    engine_instance_active: bool,
) -> Result<(), String> {
    if reducer_touched && engine_instance_active {
        Err(format!("Forge dual-write refused for story {story_id}"))
    } else {
        Ok(())
    }
}

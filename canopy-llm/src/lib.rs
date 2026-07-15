mod client;
pub use client::{LlmClient, LlmError};

mod tools;
pub use tools::{find_symbol_tool_spec, read_file_tool_spec, ChatMessage, ToolCall, ToolSpec, ToolTurn};

mod repair;

mod tech;
pub use tech::services_need_jvm;

mod skills;
pub use skills::{
    abstract_layer_for_kind, detect_layer, resolve_implementation_target, skill_for_build_system,
    skill_for_technology, skill_for_technology_all_layers, skills_for_architecture,
    testing_skill_for_file_with_adrs,
};

mod prompts;
pub use prompts::{
    audit_behavior_coverage, audit_clustering, audit_contracts, execute_implementation_step,
    execute_implementation_stub, execute_implementation_stub_with_tools,
    execute_implementation_with_test, execute_implementation_with_test_and_tools,
    extract_behaviors, extract_decisions, extract_domain_from_stories, fix_file, fix_file_with_tools,
    generate_contracts, generate_scaffold_from_services, generate_stories_from_intent,
    generate_story_openapi, generate_story_plan, generate_story_plan_from_contracts,
    generate_story_spec, generate_unit_test_stub,
    generate_unit_test_stub_with_tools, identify_architectural_questions,
    identify_specification_gaps, mechanical_cluster, parse_event_adr, propose_dependencies, review_clustering,
    suggest_domain_entities, suggest_roles, FixAttempt, StepResult,
};

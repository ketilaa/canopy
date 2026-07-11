mod client;
pub use client::{LlmClient, LlmError};

mod tools;
pub use tools::{find_symbol_tool_spec, ChatMessage, ToolCall, ToolSpec, ToolTurn};

mod repair;

mod tech;
pub use tech::services_need_jvm;

mod skills;
pub use skills::{
    detect_layer, skill_for_build_system, skill_for_technology, skill_for_technology_all_layers,
    skills_for_architecture, testing_skill_for_file_with_adrs,
};

mod prompts;
pub use prompts::{
    execute_implementation_step, execute_implementation_stub, execute_implementation_with_test,
    extract_domain_from_stories, fix_file, fix_file_with_tools, generate_scaffold_from_services,
    generate_stories_from_intent, generate_story_contract, generate_story_plan,
    generate_story_spec, generate_unit_test_stub, identify_architectural_questions,
    propose_dependencies, suggest_domain_entities, suggest_roles, FixAttempt, StepResult,
};

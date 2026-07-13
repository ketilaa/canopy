mod dependencies;
mod fix;
mod intent;
mod plan;
mod scaffold;
mod spec;
mod step;
mod summary;

pub use dependencies::propose_dependencies;
pub use fix::{fix_file, fix_file_with_tools, FixAttempt};
pub use intent::{extract_domain_from_stories, generate_stories_from_intent, suggest_domain_entities, suggest_roles};
pub use plan::generate_story_plan;
pub use scaffold::generate_scaffold_from_services;
pub use spec::{generate_story_openapi, generate_story_spec, identify_architectural_questions};
pub use step::{
    execute_implementation_step, execute_implementation_stub, execute_implementation_stub_with_tools,
    execute_implementation_with_test, execute_implementation_with_test_and_tools,
    generate_unit_test_stub, generate_unit_test_stub_with_tools,
};
pub use summary::StepResult;

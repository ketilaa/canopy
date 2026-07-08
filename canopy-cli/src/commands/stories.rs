use anyhow::{Context, Result};
use canopy_storage::load_user_stories;

fn print_stories_section(label: &str, stories: &[&canopy_core::UserStory]) {
    if stories.is_empty() { return; }
    println!("  {label} ({})", stories.len());
    for s in stories {
        println!("    [{}] As a {}, I want {}", s.id, s.as_a, s.want);
        println!("          so that {}", s.so_that);
        if !s.depends_on.is_empty() {
            println!("          depends on: {}", s.depends_on.join(", "));
        }
    }
    println!();
}
pub(crate) fn cmd_stories() -> Result<()> {
    let stories = load_user_stories().context("failed to load stories.yaml")?;

    let accepted: Vec<_> = stories.stories.iter()
        .filter(|s| s.status == canopy_core::StoryStatus::Accepted).collect();
    let draft: Vec<_> = stories.stories.iter()
        .filter(|s| s.status == canopy_core::StoryStatus::Draft).collect();
    let rejected: Vec<_> = stories.stories.iter()
        .filter(|s| s.status == canopy_core::StoryStatus::Rejected).collect();

    println!("{} user stories:\n", stories.stories.len());
    print_stories_section("Accepted", &accepted);
    print_stories_section("Draft", &draft);
    print_stories_section("Rejected", &rejected);

    if stories.stories.is_empty() {
        println!("No stories yet. Run `canopy intent` to add your first behavioral requirement.");
    } else {
        println!("Edit .canopy/stories.yaml to curate: set status to accepted | rejected.");
        println!("Run `canopy intent` to add more stories, `canopy spec <id>` to specify an accepted story.");
    }
    Ok(())
}

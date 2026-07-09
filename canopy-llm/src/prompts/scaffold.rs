use crate::tech::infer_working_dir;
use canopy_core::*;

pub fn generate_scaffold_from_services(services: &ServicesRegistry, group_id: &str) -> ScaffoldPlan {
    let mut commands = Vec::new();
    for service in &services.services {
        if service.component_type.as_deref() == Some("infrastructure") {
            eprintln!(
                "  (skipping '{}': infrastructure component — managed via docker-compose or similar)",
                service.name
            );
            continue;
        }
        if let Some(ref tech) = service.technology {
            let working_dir = service
                .component_type
                .as_deref()
                .map(|ct| if ct == "frontend" { "frontend" } else { "services" })
                .unwrap_or_else(|| infer_working_dir(tech));
            match technology_to_command(&service.name, tech, group_id, working_dir) {
                Some(cmd) => commands.push(cmd),
                None => eprintln!(
                    "  (skipping '{}': no scaffold template for '{}')",
                    service.name, tech
                ),
            }
        } else {
            eprintln!(
                "  (skipping '{}': no technology decided — run `canopy spec` to resolve tech stack ADRs)",
                service.name
            );
        }
    }
    ScaffoldPlan { generated_at: String::new(), commands }
}

fn technology_to_command(
    name: &str,
    technology: &str,
    group_id: &str,
    working_dir: &str,
) -> Option<ScaffoldCommand> {
    let t = technology.to_lowercase();
    let artifact_id = name.to_lowercase().replace(' ', "-");

    let (command, creates) = if t.contains("next.js") || t.contains("nextjs") {
        (
            format!("npx create-next-app@latest {name} --typescript --tailwind --app --no-git"),
            format!("{name}/"),
        )
    } else if t.contains("angular") {
        (
            format!("npx @angular/cli@latest new {name} --directory={name} --style=css --routing --skip-git --no-interactive"),
            format!("{name}/"),
        )
    } else if t.contains("vite")
        || t.contains("react")
        || t.contains("vue")
        || t.contains("svelte")
        || t.contains("solid")
        || t.contains("preact")
        || t.contains("lit")
    {
        let template = vite_template_for(&t);
        (
            format!(
                "printf 'n\\n' | npm create vite@latest {name} -- --template {template} && \
                 cd {name} && npm install && \
                 npm install --save-dev vitest @testing-library/react @testing-library/user-event \
                   @testing-library/jest-dom jsdom && \
                 npm pkg set scripts.test=\"vitest run\" && \
                 node -e \"var fs=require('fs');\
var v=fs.readFileSync('vite.config.ts','utf8');\
v=v.replace('plugins: [react()]','plugins: [react()],\\n  test: {{ globals: true, environment: \\\"jsdom\\\", setupFiles: [\\\"./src/test/setup.ts\\\"] }}');\
fs.writeFileSync('vite.config.ts',v);\" && \
                 mkdir -p src/test && \
                 node -e \"require('fs').writeFileSync('src/test/setup.ts','import \\\"@testing-library/jest-dom\\\";\\n');\""
            ),
            format!("{name}/"),
        )
    } else if t.contains("spring boot") || t.contains("spring-boot") {
        let (lang, proj_type) = if t.contains("kotlin") {
            ("kotlin", "gradle-project")
        } else {
            ("java", "maven-project")
        };
        (
            format!(
                "mkdir -p {artifact_id} && curl -G https://start.spring.io/starter.tgz \\\n  -d dependencies=web,actuator -d language={lang} -d type={proj_type} \\\n  -d bootVersion=4.1.0 \\\n  -d groupId={group_id} -d artifactId={artifact_id} -d name={artifact_id} \\\n  | tar -xzvf - -C {artifact_id}"
            ),
            format!("{artifact_id}/"),
        )
    } else if t.contains("node") || t.contains("express") || t.contains("fastify")
        || t.contains("koa") || t.contains("hapi")
    {
        (
            format!(
                "mkdir -p {name} && cd {name} && \
                 npm init -y && \
                 npm install express zod && \
                 npm install --save-dev typescript ts-node @types/express @types/node \
                   jest ts-jest @types/jest supertest @types/supertest && \
                 npx tsc --init --target ES2020 --lib ES2020 \
                   --esModuleInterop --skipLibCheck --resolveJsonModule && \
                 node -e \"var fs=require('fs');\
var c=new Function('return '+fs.readFileSync('tsconfig.json','utf8'))();\
c.compilerOptions.module='node16';\
c.compilerOptions.moduleResolution='node16';\
c.compilerOptions.types=['jest','node'];\
delete c.compilerOptions.verbatimModuleSyntax;\
delete c.compilerOptions.jsx;\
c.compilerOptions.isolatedModules=true;\
delete c.compilerOptions.noUncheckedSideEffectImports;\
delete c.compilerOptions.moduleDetection;\
c.include=['src/**/*'];\
c.exclude=['node_modules'];\
fs.writeFileSync('tsconfig.json',JSON.stringify(c,null,2));\" && \
                 mkdir -p src && \
                 npx --yes ts-jest config:init && \
                 npm pkg set scripts.test=\"jest --forceExit\" \
                             scripts.build=\"tsc --noEmit\" \
                             scripts.dev=\"ts-node src/index.ts\""
            ),
            format!("{name}/"),
        )
    } else if t.contains("python") || t.contains("django") || t.contains("flask") || t.contains("fastapi") {
        (
            format!("mkdir -p {name} && touch {name}/main.py {name}/requirements.txt"),
            format!("{name}/"),
        )
    } else if t.contains("rust") || t.contains("axum") || t.contains("actix") || t.contains("rocket") {
        (
            format!("cargo new {name}"),
            format!("{name}/"),
        )
    } else if t.contains(".net") || t.contains("dotnet") || t.contains("asp.net") || t.contains("c#") {
        (
            format!("dotnet new webapi -n {name}"),
            format!("{name}/"),
        )
    } else if t.contains("spring") || t.contains("java") || t.contains("maven") {
        (
            format!("mvn archetype:generate -DgroupId={group_id} -DartifactId={artifact_id} -DarchetypeArtifactId=maven-archetype-quickstart -DarchetypeVersion=1.4 -DinteractiveMode=false"),
            format!("{artifact_id}/"),
        )
    } else if t.contains("kotlin") || t.contains("gradle") {
        (
            format!("gradle init --type kotlin-application --dsl kotlin --no-incubating"),
            format!("{name}/"),
        )
    } else {
        return None;
    };

    Some(ScaffoldCommand {
        label: format!("{name} ({technology})"),
        command,
        working_dir: working_dir.to_string(),
        creates,
    })
}

fn vite_template_for(tech_lower: &str) -> &'static str {
    // Always use TypeScript variants — avoids the variant-selection prompt in Vite 8
    // when a plain JS template is specified (which interprets 'n' as cancel).
    if tech_lower.contains("react") && tech_lower.contains("swc") {
        "react-swc-ts"
    } else if tech_lower.contains("react") {
        "react-ts"
    } else if tech_lower.contains("vue") {
        "vue-ts"
    } else if tech_lower.contains("svelte") {
        "svelte-ts"
    } else if tech_lower.contains("solid") {
        "solid-ts"
    } else if tech_lower.contains("preact") {
        "preact-ts"
    } else if tech_lower.contains("lit") {
        "lit-ts"
    } else {
        "vanilla-ts"
    }
}


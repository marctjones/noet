use noet_ai::{
    ChatMessage, ChatRequest, ChatRole, ChatRuntime, LocalModelSpec, LocalRuntimeSettings,
    MistralRsInlineChatRuntime,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MODELS: &[(&str, &str, &str, &str)] = &[
    (
        "mistral7b",
        "bartowski",
        "Mistral-7B-Instruct-v0.3-GGUF",
        "Mistral-7B-Instruct-v0.3-Q4_K_M.gguf",
    ),
    (
        "ministral8b",
        "bartowski",
        "Ministral-8B-Instruct-2410-GGUF",
        "Ministral-8B-Instruct-2410-Q4_K_M.gguf",
    ),
    (
        "mistralnemo",
        "bartowski",
        "Mistral-Nemo-Instruct-2407-GGUF",
        "Mistral-Nemo-Instruct-2407-Q4_K_M.gguf",
    ),
];

const PROMPTS: &[(&str, &str, usize)] = &[
    (
        "labels",
        "Read the note and produce 4 concise labels, one per line, no bullets, no numbering, no explanation.",
        64,
    ),
    (
        "tasks",
        "Read the note and extract 4 actionable follow-up items. Keep them short, imperative, and specific. Output one item per line with no bullets and no explanation.",
        80,
    ),
    (
        "agenda",
        "Draft a useful 1:1 agenda from the note. Produce 4 agenda items, one per line, concise but concrete, no bullets and no explanation.",
        80,
    ),
    (
        "review",
        "Review the note and summarize the key decisions, risks, open questions, and follow-ups in 3 short bullet points. Keep it compact and avoid preamble.",
        24,
    ),
];

fn main() {
    let mut args = std::env::args().skip(1);
    let mut run_all = false;
    let mut model_key = None;
    let mut prompt_key = None;
    let mut model_root = default_model_root();
    let mut min_free_percent: u8 = 25;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--all" => run_all = true,
            "--model-root" => {
                model_root = PathBuf::from(args.next().expect("--model-root requires a path"));
            }
            "--min-free-percent" => {
                min_free_percent = args
                    .next()
                    .expect("--min-free-percent requires a value")
                    .parse()
                    .expect("invalid --min-free-percent");
            }
            "--help" | "-h" => {
                print_usage();
                return;
            }
            value if model_key.is_none() => model_key = Some(value.to_string()),
            value if prompt_key.is_none() => prompt_key = Some(value.to_string()),
            _ => {
                eprintln!("unexpected argument");
                print_usage();
                std::process::exit(2);
            }
        }
    }

    if let Err(message) = check_memory_pressure(min_free_percent) {
        eprintln!("{message}");
        std::process::exit(75);
    }

    let settings = runtime_settings(&model_root);
    println!("model\tprompt\tinput_tokens\toutput_tokens\telapsed_seconds\toutput_tps\tstatus");

    if run_all {
        for (model_key, _, _, _) in MODELS {
            run_model(&settings, model_key, None);
        }
        return;
    }

    let model_key = model_key.unwrap_or_else(|| {
        print_usage();
        std::process::exit(2);
    });
    let prompt_key = prompt_key.unwrap_or_else(|| {
        print_usage();
        std::process::exit(2);
    });
    run_model(&settings, &model_key, Some(&prompt_key));
}

fn run_model(settings: &LocalRuntimeSettings, model_key: &str, prompt_filter: Option<&str>) {
    let Some((_, _, _, _)) = MODELS.iter().find(|(key, _, _, _)| *key == model_key) else {
        eprintln!("unknown model key: {model_key}");
        std::process::exit(2);
    };
    let Some(spec) = settings.models.get(model_key).cloned() else {
        eprintln!("missing model spec for {model_key}");
        std::process::exit(1);
    };

    let mut model_settings = settings.clone();
    model_settings.models = BTreeMap::from([(model_key.to_string(), spec)]);
    let runtime = match MistralRsInlineChatRuntime::load(model_settings, model_key.to_string()) {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("{model_key}\tload_failed\t0\t0\t0\t0\t{:?}", err);
            std::process::exit(1);
        }
    };

    let mut total_output_tokens = 0u64;
    let mut total_elapsed_seconds = 0f64;
    let mut matched_prompt = false;
    for (prompt_key, prompt_text, max_output_tokens) in PROMPTS {
        if let Some(filter) = prompt_filter {
            if filter != *prompt_key {
                continue;
            }
        }
        matched_prompt = true;
        let request = ChatRequest {
            profile_id: model_key.to_string(),
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: prompt_text.to_string(),
            }],
            max_output_tokens: Some(*max_output_tokens as u32),
            temperature_millis: Some(0),
        };
        let started = Instant::now();
        match runtime.complete_chat(request) {
            Ok(response) => {
                let elapsed = started.elapsed().as_secs_f64();
                let input_tokens = response
                    .usage
                    .as_ref()
                    .and_then(|u| u.input_tokens)
                    .unwrap_or(0);
                let output_tokens = response
                    .usage
                    .as_ref()
                    .and_then(|u| u.output_tokens)
                    .unwrap_or(0);
                let output_tps = if elapsed > 0.0 {
                    output_tokens as f64 / elapsed
                } else {
                    0.0
                };
                total_output_tokens += u64::from(output_tokens);
                total_elapsed_seconds += elapsed;
                println!(
                    "{model_key}\t{prompt_key}\t{input_tokens}\t{output_tokens}\t{elapsed:.2}\t{output_tps:.2}\tok"
                );
            }
            Err(err) => {
                println!("{model_key}\t{prompt_key}\t0\t0\t0.00\t0.00\t{:?}", err);
            }
        }
    }

    if prompt_filter.is_none() {
        let total_tps = if total_elapsed_seconds > 0.0 {
            total_output_tokens as f64 / total_elapsed_seconds
        } else {
            0.0
        };
        println!(
            "{model_key}\t__summary__\t0\t{}\t{total_elapsed_seconds:.2}\t{total_tps:.2}\tok",
            total_output_tokens
        );
    } else if !matched_prompt {
        eprintln!("unknown prompt key: {}", prompt_filter.unwrap());
        std::process::exit(2);
    }
}

fn runtime_settings(model_root: &Path) -> LocalRuntimeSettings {
    let mut settings = LocalRuntimeSettings::embedded();
    settings.max_seq_len = 1024;
    settings.max_seqs = 1;
    settings.prefix_cache_n = 0;
    settings.timeout_seconds = 900;
    settings.models = model_specs(model_root);
    settings
}

fn model_specs(model_root: &Path) -> BTreeMap<String, LocalModelSpec> {
    MODELS
        .iter()
        .map(|(profile, owner, repo, file)| {
            (
                (*profile).to_string(),
                LocalModelSpec {
                    model_dir: resolve_model_dir(model_root, owner, repo, file),
                    quantized_file: (*file).to_string(),
                },
            )
        })
        .collect()
}

fn resolve_model_dir(model_root: &Path, owner: &str, repo: &str, file: &str) -> PathBuf {
    let direct = model_root.join(file);
    if direct.exists() {
        return model_root.to_path_buf();
    }
    let repo_dir = model_root.join(owner).join(repo);
    if repo_dir.join(file).exists() {
        return repo_dir;
    }
    let hf_repo = model_root
        .join(format!("models--{owner}--{repo}"))
        .join("snapshots");
    if let Ok(snapshots) = std::fs::read_dir(&hf_repo) {
        for snapshot in snapshots.flatten() {
            let dir = snapshot.path();
            if dir.join(file).exists() {
                return dir;
            }
        }
    }
    model_root.join(format!("models--{owner}--{repo}"))
}

fn check_memory_pressure(min_free_percent: u8) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Ok(());
    }
    let output = std::process::Command::new("memory_pressure")
        .output()
        .map_err(|err| format!("failed to run memory_pressure: {err}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let Some(free_percent) = text.lines().find_map(|line| {
        line.split_once("System-wide memory free percentage: ")
            .and_then(|(_, value)| value.trim().trim_end_matches('%').parse::<u8>().ok())
    }) else {
        return Ok(());
    };
    if free_percent < min_free_percent {
        return Err(format!(
            "memory pressure too high for local model load: {free_percent}% free, require {min_free_percent}%"
        ));
    }
    Ok(())
}

fn default_model_root() -> PathBuf {
    std::env::var_os("HF_CACHE_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HF_HOME")
                .map(PathBuf::from)
                .map(|home| home.join("hub"))
        })
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".cache").join("huggingface").join("hub"))
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

fn print_usage() {
    eprintln!(
        "usage: noet-model-bench [--all] [--model-root PATH] [--min-free-percent N] <model> <prompt>"
    );
    eprintln!("models: mistral7b, ministral8b, mistralnemo");
    eprintln!("prompts: labels, tasks, agenda, review");
}

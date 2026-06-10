//! Headless performance benchmark for Noet's backend.
//!
//! Generates a synthetic vault of N markdown notes (realistic token density:
//! workstreams, people, tags, typed todos with dates/priorities) and times the
//! operations that the UI's `refresh()` actually calls. No window, no GPU — this
//! isolates the data-layer cost that scales with vault size.
//!
//!   cargo run --release --bin noet-bench -- [N] [vault_dir]
//!
//! Defaults: N = 2000 notes, vault = a fresh temp dir (removed on start).

use noet_core::backend::{Backend, Filter};
use std::path::PathBuf;
use std::time::Instant;

// Small deterministic PRNG so runs are reproducible (no rand dependency).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 16
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[(self.next() as usize) % xs.len()]
    }
    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + (self.next() as usize) % (hi - lo + 1)
    }
}

const WORKSTREAMS: &[&str] = &[
    "Platform",
    "Platform/Auth",
    "Platform/Billing",
    "Growth",
    "Growth/SEO",
    "Hiring",
    "Hiring/Eng",
    "OKRs",
    "OKRs/Q3",
    "Infra",
    "Infra/Migration",
    "Support",
];
const PEOPLE: &[&str] = &[
    "alice", "bob", "carol", "dave", "erin", "frank", "grace", "heidi", "ivan", "judy", "mallory",
    "niaj", "olivia", "peggy", "rupert", "sybil",
];
const TAGS: &[&str] = &[
    "urgent",
    "blocked",
    "idea",
    "decision",
    "risk",
    "followup",
    "1on1",
    "roadmap",
    "bug",
    "research/ml",
    "research/ux",
    "budget",
];
const KINDS: &[&str] = &[
    "do",
    "mine",
    "followup",
    "delegated",
    "waiting",
    "someday",
    "reading",
];
const PRIOS: &[&str] = &["A", "B", "C", ""];

fn gen_note(rng: &mut Lcg, i: usize) -> String {
    let ws = rng.pick(WORKSTREAMS);
    let title = format!("Note {i} — {}", rng.pick(TAGS));
    let mut s = format!(
        "---\nid: n{i:06}\ntitle: \"{title}\"\ncreated: 2026-{:02}-{:02}\nupdated: 2026-{:02}-{:02}\nkind: note\n---\n",
        rng.range(1, 12), rng.range(1, 28), rng.range(1, 12), rng.range(1, 28),
    );
    s.push_str(&format!(
        "# {title}\n\nMeeting in [[{ws}]] with @{}.\n\n",
        rng.pick(PEOPLE)
    ));
    s.push_str(&format!(
        "Some context paragraph about #{} and #{}. ",
        rng.pick(TAGS),
        rng.pick(TAGS)
    ));
    s.push_str("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod.\n\n");
    // a handful of typed todos
    for _ in 0..rng.range(2, 6) {
        let kind = rng.pick(KINDS);
        let person = rng.pick(PEOPLE);
        let prio = rng.pick(PRIOS);
        let pstr = if prio.is_empty() {
            String::new()
        } else {
            format!(" [#{prio}]")
        };
        let due = format!(" due:2026-{:02}-{:02}", rng.range(1, 12), rng.range(1, 28));
        s.push_str(&format!(
            "- [ ] Follow up on item with @{person} [[{ws}]] #{kind} #{}{pstr}{due}\n",
            rng.pick(TAGS),
        ));
    }
    s.push('\n');
    s
}

fn time<T>(label: &str, n: usize, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let r = f();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    let per = if n > 0 {
        format!("  ({:.3} ms / 1k notes)", ms / n as f64 * 1000.0)
    } else {
        String::new()
    };
    println!("  {label:<34} {ms:>9.2} ms{per}");
    r
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let vault: PathBuf = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("noet-bench-vault"));

    let _ = std::fs::remove_dir_all(&vault);
    std::fs::create_dir_all(vault.join("notes"))?;

    println!(
        "\nNoet backend benchmark — {n} notes\n  vault: {}\n",
        vault.display()
    );

    // --- generate the synthetic vault -------------------------------------
    let mut rng = Lcg(0x9E3779B97F4A7C15);
    let mut bytes = 0usize;
    time("generate + write vault", n, || {
        for i in 0..n {
            let body = gen_note(&mut rng, i);
            bytes += body.len();
            std::fs::write(vault.join("notes").join(format!("n{i:06}.md")), &body).unwrap();
        }
    });
    println!("  ({:.1} MB on disk)\n", bytes as f64 / 1e6);

    // --- cold open (creates schema + full index) --------------------------
    let mut b = time("open() — schema + full index", n, || {
        Backend::open(vault.clone()).unwrap()
    });

    // --- warm full reindex (the file-watch / ⟳ path) ----------------------
    time("reindex_all() — full rebuild", n, || {
        b.reindex_all().unwrap()
    });

    println!("\n  --- queries that refresh() runs every interaction ---");
    let f = Filter::default();
    let cnt = time("query_notes(default)", n, || {
        b.query_notes(&f).unwrap().len()
    });
    println!("    -> {cnt} notes");
    time("query_notes(search='roadmap')", n, || {
        let sf = Filter {
            search: "roadmap".into(),
            ..Default::default()
        };
        b.query_notes(&sf).unwrap().len()
    });
    let tc = time("query_todos(default)", n, || {
        b.query_todos(&f).unwrap().len()
    });
    println!("    -> {tc} todos");
    time("query_todos(status=open)", n, || {
        let sf = Filter {
            status: "open".into(),
            ..Default::default()
        };
        b.query_todos(&sf).unwrap().len()
    });
    time("board(project)", n, || {
        b.board("project", &f).unwrap().len()
    });
    time("agenda()", n, || b.agenda(&f).unwrap().len());
    time("gantt_items()", n, || b.gantt_items(&f).unwrap().len());
    time("list_projects()", n, || b.list_projects().unwrap().len());
    time("list_tags()", n, || b.list_tags().unwrap().len());
    time("list_people()", n, || b.list_people().unwrap().len());
    time("inbox()", n, || b.inbox().unwrap().len());
    time("stale_todos()", n, || b.stale_todos().unwrap().len());

    // --- one full refresh() worth of work (sum of a tab switch) -----------
    println!("\n  --- one full refresh() (every view, as today) ---");
    time("ALL queries (simulated refresh)", n, || {
        let _ = b.query_notes(&f).unwrap();
        let _ = b.query_todos(&f).unwrap();
        let _ = b.board("project", &f).unwrap();
        let _ = b.agenda(&f).unwrap();
        let _ = b.gantt_items(&f).unwrap();
        let _ = b.list_projects().unwrap();
        let _ = b.list_tags().unwrap();
        let _ = b.list_people().unwrap();
        let _ = b.inbox().unwrap();
        let _ = b.stale_todos().unwrap();
    });

    println!("\ndone.\n");
    Ok(())
}

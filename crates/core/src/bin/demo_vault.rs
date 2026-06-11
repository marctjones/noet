//! Generate a deterministic Noet demo vault.
//!
//! Run through `scripts/reset-demo-vault.sh` so stale demo data is removed first.

use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

struct Person {
    name: &'static str,
    role: &'static str,
    workstream: &'static str,
    link: &'static str,
}

const DIRECT_REPORTS: &[Person] = &[
    Person {
        name: "Ava Chen",
        role: "platform engineering lead",
        workstream: "workstream/enterprise-saas",
        link: "OneTrust rollout",
    },
    Person {
        name: "Mateo Alvarez",
        role: "SaaS integrations engineer",
        workstream: "workstream/onetrust",
        link: "OneTrust data map",
    },
    Person {
        name: "Priya Nair",
        role: "AI policy counsel",
        workstream: "workstream/ai-law",
        link: "AI model release checklist",
    },
    Person {
        name: "Owen Brooks",
        role: "open source security engineer",
        workstream: "workstream/open-source-security",
        link: "Open source intake policy",
    },
    Person {
        name: "Lila Morgan",
        role: "model security researcher",
        workstream: "workstream/model-security",
        link: "Model security red-team notes",
    },
    Person {
        name: "Jamal Carter",
        role: "enterprise applications engineer",
        workstream: "workstream/credo-ai",
        link: "Credo AI risk taxonomy",
    },
    Person {
        name: "Nora Weiss",
        role: "research operations PM",
        workstream: "workstream/research-operations",
        link: "Research review cadence",
    },
];

const COLLABORATORS: &[Person] = &[
    Person {
        name: "Elena Rossi",
        role: "general counsel",
        workstream: "workstream/ai-law",
        link: "AI law weekly",
    },
    Person {
        name: "Victor Huang",
        role: "CISO",
        workstream: "workstream/model-security",
        link: "Model security red-team notes",
    },
    Person {
        name: "Sarah Patel",
        role: "procurement lead",
        workstream: "workstream/enterprise-saas",
        link: "OneTrust rollout",
    },
    Person {
        name: "Ben Okafor",
        role: "OneTrust product owner",
        workstream: "workstream/onetrust",
        link: "OneTrust data map",
    },
    Person {
        name: "Maya Schneider",
        role: "Credo AI program lead",
        workstream: "workstream/credo-ai",
        link: "Credo AI risk taxonomy",
    },
    Person {
        name: "Kira Sato",
        role: "AI research scientist",
        workstream: "workstream/research-operations",
        link: "Research review cadence",
    },
    Person {
        name: "Theo Martin",
        role: "open source maintainer",
        workstream: "workstream/open-source-security",
        link: "Open source intake policy",
    },
    Person {
        name: "Allison Reed",
        role: "privacy counsel",
        workstream: "workstream/ai-law",
        link: "Privacy review notes",
    },
    Person {
        name: "Daniel Kim",
        role: "customer trust lead",
        workstream: "workstream/customer-trust",
        link: "Customer trust launch packet",
    },
    Person {
        name: "Helena Duarte",
        role: "outside AI regulatory counsel",
        workstream: "workstream/ai-law",
        link: "EU AI Act mapping",
    },
];

const WORKSTREAMS: &[(&str, &str, &str)] = &[
    (
        "workstream/enterprise-saas",
        "Enterprise SaaS operating model",
        "OneTrust, Credo AI, access reviews, and enterprise application governance.",
    ),
    (
        "workstream/onetrust",
        "OneTrust rollout",
        "Privacy workflow configuration, data maps, and intake quality.",
    ),
    (
        "workstream/credo-ai",
        "Credo AI risk taxonomy",
        "AI governance workflows, model inventory, and risk taxonomy design.",
    ),
    (
        "workstream/ai-law",
        "AI law weekly",
        "Regulatory tracking, customer-facing commitments, and counsel follow-up.",
    ),
    (
        "workstream/model-security",
        "Model security red-team notes",
        "Model release gates, misuse cases, jailbreak findings, and eval coverage.",
    ),
    (
        "workstream/open-source-security",
        "Open source intake policy",
        "Dependency review, maintainer outreach, SBOMs, and disclosure handling.",
    ),
    (
        "workstream/research-operations",
        "Research review cadence",
        "Research team operating rhythm, publication review, and handoffs.",
    ),
    (
        "workstream/customer-trust",
        "Customer trust launch packet",
        "Customer security questionnaires, AI trust materials, and launch readiness.",
    ),
];

const COLLABORATOR_FOLLOWUP_DUE: &[&str] = &[
    "2026-06-18",
    "2026-06-19",
    "2026-06-20",
    "2026-06-21",
    "2026-06-22",
    "2026-06-23",
    "2026-06-24",
    "2026-06-25",
    "2026-06-26",
    "2026-06-27",
];

const COLLABORATOR_WAITING_DUE: &[&str] = &[
    "2026-06-22",
    "2026-06-23",
    "2026-06-24",
    "2026-06-25",
    "2026-06-26",
    "2026-06-27",
    "2026-06-28",
    "2026-06-29",
    "2026-06-30",
    "2026-07-01",
];

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let vault = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/noet-demo-vault"));
    let force = args.any(|arg| arg == "--force");
    reset_guard(&vault, force)?;
    if vault.exists() {
        std::fs::remove_dir_all(&vault)?;
    }
    std::fs::create_dir_all(vault.join("notes/archive"))?;
    std::fs::create_dir_all(vault.join(".trash"))?;

    write_workstream_notes(&vault)?;
    write_one_on_ones(&vault)?;
    write_collaborator_meetings(&vault)?;
    write_decisions_and_research(&vault)?;
    write_promoted_tasks(&vault)?;
    write_archived_and_trash(&vault)?;

    println!("Generated Noet demo vault at {}", vault.display());
    Ok(())
}

fn reset_guard(vault: &Path, force: bool) -> Result<()> {
    if force {
        return Ok(());
    }
    let Some(name) = vault.file_name().and_then(|name| name.to_str()) else {
        bail!("Refusing to reset a path without a final directory name");
    };
    if name.contains("noet-demo-vault") {
        Ok(())
    } else {
        bail!(
            "Refusing to reset '{}'. Use --force for non-demo paths.",
            vault.display()
        )
    }
}

fn write_workstream_notes(vault: &Path) -> Result<()> {
    for (idx, (label, title, summary)) in WORKSTREAMS.iter().enumerate() {
        let body = format!(
            "# {title}\n\n#{label}\n\n{summary}\n\n## Active Links\n\n- [[OneTrust rollout]]\n- [[Credo AI risk taxonomy]]\n- [[AI model release checklist]]\n\n## Tasks\n\n- [ ] Refresh stakeholder map #{label} #mine due:2026-06-18 priority:B\n- [ ] Review stale commitments #{label} #waiting due:2026-06-20\n"
        );
        write_note(
            vault,
            &format!("workstream-{idx:02}"),
            title,
            "2026-06-01T09:00:00",
            &body,
        )?;
    }
    Ok(())
}

fn write_one_on_ones(vault: &Path) -> Result<()> {
    let dates = ["2026-05-21", "2026-05-28", "2026-06-04", "2026-06-11"];
    for person in DIRECT_REPORTS {
        let slug = slug(person.name);
        for (idx, date) in dates.iter().enumerate() {
            let current = idx == dates.len() - 1;
            let title = if current {
                format!("1:1 - {} - current", person.name)
            } else {
                format!("1:1 - {} - {date}", person.name)
            };
            let marker = if idx == 0 { "x" } else { " " };
            let body = format!(
                "# {title}\n\n#meeting/one-on-one\n#{}\n@[[{}]]\n[[{}]]\n\n## Updates\n\n{} is focused on {}.\n\n## To Discuss\n\n- [{marker}] Follow up on last week's open item @[[{}]] #followup #{} due:2026-06-{}\n- [ ] Ask about delivery risk for [[{}]] @[[{}]] #followup #{} priority:A due:2026-06-{}\n- [ ] Delegate draft status note to {} @[[{}]] #delegated #{} due:2026-06-{}\n",
                person.workstream,
                person.name,
                person.link,
                person.name,
                person.role,
                person.name,
                person.workstream,
                12 + idx,
                person.link,
                person.name,
                person.workstream,
                16 + idx,
                person.name.split_whitespace().next().unwrap_or(person.name),
                person.name,
                person.workstream,
                18 + idx,
            );
            write_note(
                vault,
                &format!("one-on-one-{slug}-{idx}"),
                &title,
                &format!("{date}T09:00:00"),
                &body,
            )?;
        }
    }
    Ok(())
}

fn write_collaborator_meetings(vault: &Path) -> Result<()> {
    for (idx, person) in COLLABORATORS.iter().enumerate() {
        let title = format!("Meeting - {} - {}", person.name, person.link);
        let day = 1 + idx;
        let followup_due = COLLABORATOR_FOLLOWUP_DUE[idx];
        let waiting_due = COLLABORATOR_WAITING_DUE[idx];
        let body = format!(
            "# {title}\n\n#meeting\n#{}\n@[[{}]]\n[[{}]]\n\n{} joins as {}.\n\nContact: {}@example.test\n\n## Follow Ups\n\n- [ ] Send summary to {} @[[{}]] #followup #{} due:{followup_due}\n- [ ] Wait for {} input on [[{}]] @[[{}]] #waiting #{} due:{waiting_due}\n",
            person.workstream,
            person.name,
            person.link,
            person.name,
            person.role,
            slug(person.name),
            person.name,
            person.name,
            person.workstream,
            person.name.split_whitespace().next().unwrap_or(person.name),
            person.link,
            person.name,
            person.workstream,
        );
        write_note(
            vault,
            &format!("meeting-collaborator-{idx:02}"),
            &title,
            &format!("2026-06-{day:02}T13:00:00"),
            &body,
        )?;
    }
    Ok(())
}

fn write_decisions_and_research(vault: &Path) -> Result<()> {
    let notes = [
        (
            "decision-model-release",
            "Decision - model release gate",
            "workstream/model-security",
            "AI model release checklist",
            "Adopt a release gate that requires misuse-case review, eval evidence, and counsel sign-off.",
        ),
        (
            "decision-onetrust-intake",
            "Decision - OneTrust intake quality",
            "workstream/onetrust",
            "OneTrust rollout",
            "Move privacy intake quality checks earlier so bad requests do not reach legal review.",
        ),
        (
            "research-open-source-ai",
            "Research - open source AI obligations",
            "workstream/open-source-security",
            "Open source intake policy",
            "Track license, model card, security disclosure, and maintainer response obligations.",
        ),
        (
            "research-ai-law",
            "Research - AI law customer commitments",
            "workstream/ai-law",
            "EU AI Act mapping",
            "Map likely customer commitments against regulatory duties and internal controls.",
        ),
        (
            "vendor-credo-ai",
            "Vendor - Credo AI implementation notes",
            "workstream/credo-ai",
            "Credo AI risk taxonomy",
            "Capture taxonomy decisions, workflow blockers, and model inventory gaps.",
        ),
        (
            "customer-trust",
            "Customer trust launch packet",
            "workstream/customer-trust",
            "AI model release checklist",
            "Assemble customer-facing AI security, privacy, and governance materials.",
        ),
    ];

    for (idx, (id, title, label, link, summary)) in notes.iter().enumerate() {
        let body = format!(
            "# {title}\n\n#{label}\n#decision\n[[{link}]]\n\n{summary}\n\n- [ ] Review implications with @[[Elena Rossi]] #followup #{label} priority:A due:2026-06-{}\n- [ ] Capture implementation task for @[[Nora Weiss]] #delegated #{label} due:2026-06-{}\n",
            17 + idx,
            24 + idx,
        );
        write_note(
            vault,
            id,
            title,
            &format!("2026-06-{:02}T15:00:00", 3 + idx),
            &body,
        )?;
    }
    Ok(())
}

fn write_promoted_tasks(vault: &Path) -> Result<()> {
    let promoted = [
        (
            "task-ai-release-checklist",
            "Review AI model release checklist",
            "AI model release checklist",
            "release-checklist",
            "workstream/model-security",
            "Lila Morgan",
        ),
        (
            "task-onetrust-data-map",
            "Close OneTrust data map gaps",
            "OneTrust data map",
            "data-map-gaps",
            "workstream/onetrust",
            "Mateo Alvarez",
        ),
        (
            "task-open-source-policy",
            "Update open source intake policy",
            "Open source intake policy",
            "intake-policy",
            "workstream/open-source-security",
            "Owen Brooks",
        ),
        (
            "task-credo-taxonomy",
            "Normalize Credo AI risk taxonomy",
            "Credo AI risk taxonomy",
            "risk-taxonomy",
            "workstream/credo-ai",
            "Jamal Carter",
        ),
        (
            "task-ai-law-brief",
            "Prepare AI law customer brief",
            "AI law weekly",
            "customer-brief",
            "workstream/ai-law",
            "Priya Nair",
        ),
    ];

    for (idx, (id, title, source, anchor, label, person)) in promoted.iter().enumerate() {
        let body = format!(
            "# {title}\n\n#task\n#{label}\n@[[{person}]]\nsource:[[{}#^{}]]\n\n- [ ] {title} @[[{person}]] #mine #{label} priority:A due:2026-06-{}\n\n## Context\n\nPromoted from [[{}]] after a meeting follow-up.\n",
            source,
            anchor,
            20 + idx,
            source,
        );
        write_note(
            vault,
            id,
            title,
            &format!("2026-06-{:02}T16:00:00", 5 + idx),
            &body,
        )?;
    }
    Ok(())
}

fn write_archived_and_trash(vault: &Path) -> Result<()> {
    write_note_at(
        &vault.join("notes/archive/archive-old-onetrust.md"),
        "Archived - old OneTrust checklist",
        "2026-04-01T09:00:00",
        "# Archived - old OneTrust checklist\n\n#workstream/onetrust\nOld checklist retained for reference.\n",
    )?;
    write_note_at(
        &vault.join("notes/archive/archive-old-ai-law.md"),
        "Archived - old AI law notes",
        "2026-04-02T09:00:00",
        "# Archived - old AI law notes\n\n#workstream/ai-law\nSuperseded by [[EU AI Act mapping]].\n",
    )?;
    write_note_at(
        &vault.join(".trash/trash-duplicate-meeting.md"),
        "Trash - duplicate meeting",
        "2026-06-01T08:00:00",
        "# Trash - duplicate meeting\n\nDuplicate notes from a test import.\n",
    )?;
    write_note_at(
        &vault.join(".trash/trash-empty-capture.md"),
        "Trash - empty capture",
        "2026-06-02T08:00:00",
        "# Trash - empty capture\n\nEmpty scratch note.\n",
    )?;
    Ok(())
}

fn write_note(vault: &Path, id: &str, title: &str, updated: &str, body: &str) -> Result<()> {
    let path = vault.join("notes").join(format!("{id}.md"));
    write_note_at(&path, title, updated, body)
}

fn write_note_at(path: &Path, _title: &str, updated: &str, body: &str) -> Result<()> {
    let contents =
        format!("---\ncreated: {updated}\nupdated: {updated}\nkind: markdown\n---\n{body}");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    Ok(())
}

fn slug(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

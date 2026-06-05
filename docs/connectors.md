# Connectors — architecture, lessons, and roadmap

How Noet talks to external systems (email, tasks, lists), what's hard and why,
and a concrete plan tailored to the developer's actual environment. This is a
design/reference doc — see `docs/outlook-connector.md` for the one connector
that's deeply specced.

## Context (the constraints that shape every decision)

- **Dev machine:** personal **Linux** laptop. Core logic and any cloud/HTTP
  connector are developed and tested here.
- **Personal accounts:** **Gmail** on a **paid Google Workspace that the
  developer administers**. Crucially, *you are your own Workspace admin* — so the
  "third-party app allow-list" gate is yours to open. No external IT involved.
- **Work accounts:** **Outlook + SharePoint Online**, used only from a
  **Windows 11 work laptop** under corporate **Entra ID / IT**. You do **not**
  control that tenant's admin policy.
- **Hard requirement:** **avoid needing approval from the employer's corporate
  IT.** Preference order:
  1. **Best** — piggyback the existing *local* login (no re-login, no
     registration, no IT).
  2. **Tolerable** — register your own app with the upstream vendor (Azure /
     Google), as long as the employer's IT isn't in the loop.
  3. **Avoid** — anything that needs the employer's IT to provision/approve.

## Core architecture (already in place)

- `noet-core/connectors/<service>.rs` — each connector is: a **config** struct
  persisted in the **OS config dir** (never the vault, never the repo), **pure,
  testable logic** (parse / map / reconcile), and **thin IO**.
- **Two IO mechanisms:**
  - **Cloud REST** over HTTPS (`ureq`) — Jira today; Gmail/Todoist/Monday/Graph
    in future.
  - **Local app / OS bridge** — automate the already-signed-in desktop app:
    **COM via PowerShell** (Windows, Classic Outlook — built), **AppleScript via
    `osascript`** (macOS, future), or an **embedded WebView** (web-only services).
- **Optional + graceful + `#[cfg]`-gated:** connectors compile everywhere and
  no-op (with a clear error) where unsupported. **Pure logic is unit-tested on
  every OS including the Linux dev box; platform IO only runs on its own OS** and
  is compile-checked by the Windows CI runner.
- **External-ref convention:** todos carry tokens like `src:outlook:<id>`,
  `jira:KEY-1`, `gh:owner/repo#1`; `connectors::resolve_external_url` + an
  open-in-app handler turn them into clickable links.

## Lessons learned (the decision rules)

1. **Only two ways to "piggyback without IT":** (a) **automate the local app**
   (COM / AppleScript / UI Automation) — rides your existing login, no
   registration, no IT; (b) **Windows WAM silent SSO** — removes the *re-login*
   but still needs a `client_id` and is bound by the tenant's **consent policy**
   (so it's IT-bound on a corporate tenant). Reusing another app's **token cache
   or browser cookies** directly is audience-scoped, DPAPI-encrypted, undocumented,
   fragile, and ToS-gray — **don't**.
2. **Auth ladder** (rising effort / gatekeeper):
   personal API token (Todoist/Monday/Jira) → your-own-OAuth-app on an account
   **you** administer (your Workspace Gmail) → interactive OAuth on a **corporate**
   tenant (needs user-consent enabled, else admin) → **admin-provisioned**
   enterprise API (OneTrust/Credo) → corporate IT.
3. **The real blocker is org policy, not code.** Personal tokens and
   accounts-you-admin sidestep it entirely.
4. **Integrated Windows Auth (Kerberos/NTLM) is on-prem only.** Any HTTP client
   can do it with your Windows logon (`sspi`/WinHTTP) — browsers aren't special.
   **SharePoint *Online* is NOT IWA** — it's Azure AD cookies + the device's
   Primary Refresh Token, which you can't cleanly reuse.
5. **SharePoint Online has nothing local to piggyback** — *lists* don't sync to
   disk (document *libraries* do, as OneDrive files). So lists need either Graph
   (→ corporate IT) or an embedded browser.
6. **An embedded WebView is a "universal piggyback"** for any web service you can
   log into — but it's bounded by the **Same-Origin Policy** (you must run on each
   service's own origin and can only call *that origin's* API; you can't call
   `graph.microsoft.com` cross-origin), it leans on **undocumented internal web
   APIs** (unstable), the engine is **OS-specific** (WebView2 = Windows), and
   automating corporate web apps is **ToS-gray**. Use it **only** for web-only,
   no-clean-API, no-local-app services.
7. **Use the right tool per service.** For Outlook on Windows, **COM beats
   WebView** (documented, stable object model vs. undocumented OWA endpoints).
8. **MS To Do ≈ flagged mail + (historically) the Outlook Tasks folder**, so it's
   partly reachable through the existing COM bridge without anything new.
9. **Mac is mostly free:** Slint + `noet-core` are portable, so the build ports
   easily; the only per-OS work is the local bridge (`osascript` → Outlook-for-Mac
   mirrors the Windows COM design).

## Per-connector plan (for *this* environment)

Legend — **IT gate?** is evaluated for *your* situation (personal Workspace you
admin; corporate Entra you don't).

| Connector | Mechanism | Runs on | Auth | IT gate? | Status |
|---|---|---|---|---|---|
| **Jira** | Cloud REST | any OS | personal API token / PAT | No | **Done** |
| **Outlook mail/calendar** | COM (PowerShell) | Windows work laptop | existing profile | **No** | **Done** |
| **Outlook → Tasks (MS To Do)** | COM (extend) | Windows work laptop | existing profile | **No** | Next |
| **Gmail / Workspace** | Cloud REST (Gmail API) | **Linux dev + any** | your own OAuth app, **Internal** to your Workspace | **No** (you're the admin) | Planned |
| **Todoist** | Cloud REST | any OS | personal API token | No (if personal acct) | Planned |
| **Monday.com** | Cloud GraphQL | any OS | personal API token | Usually no | Optional |
| **SharePoint Online lists** | **Embedded WebView2** | Windows work laptop | rides Edge SSO (cookies) | **No** (but fragile) | Later, if needed |
| **MS To Do / SharePoint via Graph** | Cloud REST (Graph) | any OS | OAuth on the corporate tenant | **Yes** (admin consent) | Avoid |
| **OneTrust / Credo** | Cloud REST | any OS | admin-provisioned API client | **Yes** | Out of scope |

### Notes per connector

- **Gmail / Workspace (your best cloud connector).** Because **you administer your
  own Workspace**, you can register an OAuth app and mark it **Internal** to your
  org — which **exempts it from Google's verification / restricted-scope CASA
  assessment** that normally blocks reading mail bodies. Native-app OAuth
  (loopback `http://localhost:<port>` + PKCE, RFC 8252), token + refresh stored in
  the config dir. **Fully developable and testable on the Linux laptop.** This is
  the highest-value connector you fully control.
- **Outlook (work).** Keep COM — it rides the signed-in Classic Outlook profile,
  no IT, no re-login. Extend the existing flagged/category sync to the **Tasks**
  folder to cover To Do. (Compiles on Linux as a graceful no-op; runs on the work
  laptop; CI Windows runner compile-checks it.)
- **SharePoint Online lists (work).** The only no-IT path is an **embedded
  WebView2** that loads `*.sharepoint.com` (already signed in via Edge SSO) and
  calls the list REST API (`/_api/web/lists/...`) via injected `fetch`. Windows-
  only, depends on session cookies + internal APIs, ToS-gray — so build it **last
  and only if you actually need list data**. Graph would be cleaner but needs
  corporate admin consent (the thing we're avoiding).
- **Todoist / Monday.** Personal API tokens, plain REST/GraphQL, any OS, no IT —
  easy wins if you use them. Todoist maps almost 1:1 onto Noet's typed todos.
- **OneTrust / Credo.** Enterprise GRC; API access is **admin-provisioned by
  design** and there's no local app to piggyback. Out of scope unless your org
  provisions an API client for you.

## Build / test implications of the Linux-dev ↔ Windows-work split

- **Develop on Linux:** core + the **Gmail** connector are fully exercisable here.
- **Windows-only connectors (Outlook COM, SharePoint WebView2):** write them
  `#[cfg(windows)]` + graceful elsewhere; **unit-test the pure parts on Linux**
  (parsing, mapping, reconciliation), and exercise the actual COM/WebView IO only
  on the **work laptop**. The Windows CI runner keeps them compiling.
- **Keep work and personal separate:** no work credentials or data in the repo or
  the Linux dev environment; work connectors run only on the work machine.

## Suggested sequencing

1. **Shared native-app OAuth helper** (loopback + PKCE + token store/refresh) —
   build once; Gmail and any future your-own-app OAuth reuse it.
2. **Gmail / Workspace** — your mail, no IT gate, Linux-testable, "Internal" app
   avoids Google verification. Biggest personal payoff.
3. **Outlook → Tasks (To Do)** — small extension of the COM bridge [work laptop].
4. **Todoist** — quick win if you use it.
5. **SharePoint Online lists via WebView2** — only if you need list data; highest
   fragility, do it knowingly [work laptop].
6. **Monday** — if you use it.
7. **Defer / avoid:** corporate Graph, OneTrust, Credo (IT-gated); a macOS build +
   `osascript` Outlook bridge (until you want Mac).

## Security / policy guardrails

- Credentials and tokens live in the **OS config dir**, never the vault, never the
  repo. (`Settings`, `JiraConfig` already follow this; Gmail tokens will too.)
- **Automating an app you're licensed to use** (Outlook COM / Outlook-for-Mac
  AppleScript) is defensible. **Extracting another app's tokens or replaying
  browser cookies is not** — avoid even when technically possible.
- Restricted-scope **Google verification only matters for *public* distribution**;
  an Internal app in your own Workspace is exempt.

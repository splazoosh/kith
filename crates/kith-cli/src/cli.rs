//! The `clap` derive command tree, global flags, and value-parsers.
//!
//! Value-parsers (e.g. [`parse_sex`]) delegate to `kith-core`'s existing
//! `FromStr` impls, so no `clap::ValueEnum` or other clap type ever lands in
//! `kith-core`.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use kith_core::prelude::{
    ChartMode, ChildRelation, CitationSubject, Confidence, EventId, EventKind, EventSubject,
    FamilyId, GenealogicalDate, MediaSubject, NameKind, PersonId, Sex, Theme, UnionType,
};

/// `kith` — a local-first family-tree manager.
#[derive(Debug, Parser)]
#[command(name = "kith", version, about = "Local-first family-tree manager")]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,
    #[command(subcommand)]
    pub command: Command,
}

/// Flags accepted before or after any subcommand.
#[derive(Debug, Args)]
pub struct GlobalArgs {
    /// Database file to use (defaults to the per-user data directory).
    #[arg(long, global = true, value_name = "PATH")]
    pub db: Option<PathBuf>,
    /// Emit machine-readable JSON instead of human-readable tables.
    #[arg(long, global = true)]
    pub json: bool,
    /// Increase diagnostic verbosity (repeatable: `-v`, `-vv`).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
    /// Suppress success/confirmation lines (queried data still prints).
    #[arg(long, global = true)]
    pub quiet: bool,
}

/// The top-level command.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create the database and run migrations.
    Init,
    /// Manage individuals.
    Person {
        #[command(subcommand)]
        command: PersonCommand,
    },
    /// Manage an individual's alternate names.
    Name {
        #[command(subcommand)]
        command: NameCommand,
    },
    /// Manage families/unions and their members.
    Family {
        #[command(subcommand)]
        command: FamilyCommand,
    },
    /// Record and manage events (births, deaths, marriages, …).
    Event {
        #[command(subcommand)]
        command: EventCommand,
    },
    /// Walk a person's relatives (ancestors or descendants).
    Query {
        #[command(subcommand)]
        command: QueryCommand,
    },
    /// Database maintenance: backup, restore, vacuum.
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    /// Export a chart to HTML or the whole tree to GEDCOM (`export html|gedcom`).
    Export {
        #[command(subcommand)]
        command: ExportCommand,
    },
    /// Import data into the database (`import gedcom`; creates a fresh DB by default).
    Import {
        #[command(subcommand)]
        command: ImportCommand,
    },
    /// Manage a subject's photos/media.
    Media {
        #[command(subcommand)]
        command: MediaCommand,
    },
    /// Manage sources (evidence: books, registers, record sets).
    Source {
        #[command(subcommand)]
        command: SourceCommand,
    },
    /// Attach / manage citations linking a source to a fact.
    Citation {
        #[command(subcommand)]
        command: CitationCommand,
    },
    /// Search individuals by name, alternate name, nickname, note, or place.
    Search(SearchArgs),
    /// Developer tools (hidden; built only with `--features dev`).
    #[cfg(feature = "dev")]
    Dev {
        #[command(subcommand)]
        command: DevCommand,
    },
}

/// `dev` subcommands — dev-only utilities gated behind the `dev` feature, absent
/// from the released binary.
#[cfg(feature = "dev")]
#[derive(Debug, Subcommand)]
pub enum DevCommand {
    /// Seed the database with a deterministic synthetic pedigree for manual
    /// profiling (over `kith_core::synth`).
    Seed(DevSeedArgs),
}

/// Arguments for `dev seed`: build a synthetic tree of about `--individuals`
/// people from a fixed `--seed`, printing the focal id to root a chart on.
#[cfg(feature = "dev")]
#[derive(Debug, Args)]
pub struct DevSeedArgs {
    /// Approximate number of individuals to generate.
    #[arg(long, default_value_t = 5000)]
    pub individuals: u32,
    /// PRNG seed — a fixed value reproduces the same structure.
    #[arg(long, default_value_t = 0x5EED_C0DE)]
    pub seed: u64,
}

/// Arguments for `search`: a ranked, multi-field people search. A
/// flat noun (not under `query`, which is the relationship walks) — searching is
/// "find a person", a different verb. The global `--json` drives JSON output.
#[derive(Debug, Args)]
pub struct SearchArgs {
    /// The text to find. Matches names, alternate names, nickname, notes, and
    /// the place names of a person's events; an empty query lists everyone.
    pub query: String,
    /// Maximum number of hits to return (ranked best-match-first).
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
}

/// `source` subcommands: create / list / show / edit / remove sources. Deleting a
/// source cascades its citations.
#[derive(Debug, Subcommand)]
pub enum SourceCommand {
    /// Create a source.
    Add(SourceAddArgs),
    /// List all sources.
    List,
    /// Show one source with the facts it supports.
    Show(SourceShowArgs),
    /// Edit a source; only the flags you pass are changed.
    Edit(SourceEditArgs),
    /// Remove a source (its citations cascade).
    Rm(SourceRmArgs),
}

/// Arguments for `source add`. Only `--title` is required.
#[derive(Debug, Args)]
pub struct SourceAddArgs {
    /// Title of the source (required).
    #[arg(long)]
    pub title: String,
    /// Author / originator.
    #[arg(long)]
    pub author: Option<String>,
    /// Publication facts.
    #[arg(long)]
    pub publication: Option<String>,
    /// Holding repository (a free-text name).
    #[arg(long)]
    pub repository: Option<String>,
    /// Free-form notes.
    #[arg(long)]
    pub notes: Option<String>,
}

/// Arguments for `source show`.
#[derive(Debug, Args)]
pub struct SourceShowArgs {
    /// The source's id.
    pub id: i64,
}

/// Arguments for `source edit`. Every field is optional: an absent flag leaves the
/// column unchanged.
#[derive(Debug, Args)]
pub struct SourceEditArgs {
    /// The source's id.
    pub id: i64,
    /// New title.
    #[arg(long)]
    pub title: Option<String>,
    /// New author.
    #[arg(long)]
    pub author: Option<String>,
    /// New publication facts.
    #[arg(long)]
    pub publication: Option<String>,
    /// New repository.
    #[arg(long)]
    pub repository: Option<String>,
    /// New notes.
    #[arg(long)]
    pub notes: Option<String>,
}

/// Arguments for `source rm`.
#[derive(Debug, Args)]
pub struct SourceRmArgs {
    /// The source's id.
    pub id: i64,
}

/// `citation` subcommands: attach / list / remove citations. Citations attach to a
/// fact subject (`event:<id>`, `person:<id>`, or `family:<id>`).
#[derive(Debug, Subcommand)]
pub enum CitationCommand {
    /// Attach a citation linking a source to a fact.
    Add(CitationAddArgs),
    /// List a subject's citations (with their resolved source).
    List(CitationListArgs),
    /// Remove a citation by id.
    Rm(CitationRmArgs),
}

/// Arguments for `citation add`.
#[derive(Debug, Args)]
pub struct CitationAddArgs {
    /// The source being cited.
    #[arg(long, value_name = "ID")]
    pub source: i64,
    /// The fact to cite: `event:<id>`, `person:<id>`, or `family:<id>`.
    #[arg(long, value_parser = parse_citation_subject)]
    pub subject: CitationSubject,
    /// Where in the source (page / entry).
    #[arg(long)]
    pub page: Option<String>,
    /// Confidence: `primary`, `secondary`, or `questionable`.
    #[arg(long, value_parser = parse_confidence)]
    pub confidence: Option<Confidence>,
    /// A transcription / extra detail.
    #[arg(long)]
    pub detail: Option<String>,
}

/// Arguments for `citation list`.
#[derive(Debug, Args)]
pub struct CitationListArgs {
    /// The subject: `event:<id>`, `person:<id>`, or `family:<id>`.
    #[arg(value_parser = parse_citation_subject)]
    pub subject: CitationSubject,
}

/// Arguments for `citation rm`.
#[derive(Debug, Args)]
pub struct CitationRmArgs {
    /// The citation's id.
    pub id: i64,
}

/// `media` subcommands: attach / list / re-primary / remove a subject's photos.
/// Files are copied into the media folder beside the DB (`<db-stem>.media/`).
#[derive(Debug, Subcommand)]
pub enum MediaCommand {
    /// Attach an image to a subject, copying it into the media folder.
    Add(MediaAddArgs),
    /// List a subject's media (primary first).
    List(MediaListArgs),
    /// Make a media item the subject's primary (portrait).
    SetPrimary(MediaSetPrimaryArgs),
    /// Remove a media row (its links cascade; the on-disk file is left).
    Rm(MediaRmArgs),
}

/// Arguments for `media add`.
#[derive(Debug, Args)]
pub struct MediaAddArgs {
    /// The subject: `person:<id>`, `family:<id>`, or `event:<id>`.
    #[arg(value_parser = parse_media_subject)]
    pub subject: MediaSubject,
    /// The image file to import (jpg/jpeg/png/gif/webp).
    pub file: PathBuf,
    /// Mark it the subject's primary (portrait), clearing any prior primary.
    #[arg(long)]
    pub primary: bool,
}

/// Arguments for `media list`.
#[derive(Debug, Args)]
pub struct MediaListArgs {
    /// The subject: `person:<id>`, `family:<id>`, or `event:<id>`.
    #[arg(value_parser = parse_media_subject)]
    pub subject: MediaSubject,
}

/// Arguments for `media set-primary`.
#[derive(Debug, Args)]
pub struct MediaSetPrimaryArgs {
    /// The media id to promote.
    pub media: i64,
    /// The subject it is linked to: `person:<id>`, `family:<id>`, or `event:<id>`.
    #[arg(value_parser = parse_media_subject)]
    pub subject: MediaSubject,
}

/// Arguments for `media rm`.
#[derive(Debug, Args)]
pub struct MediaRmArgs {
    /// The media id to delete.
    pub media: i64,
}

/// `export` subcommands. Render/serialize the database to a shareable file. The
/// sub-noun layer mirrors `db backup|restore|vacuum`.
#[derive(Debug, Subcommand)]
pub enum ExportCommand {
    /// Render a chart to a single self-contained `.html` file.
    Html(ExportHtmlArgs),
    /// Serialize the whole database to a GEDCOM 5.5.1 file.
    Gedcom(ExportGedcomArgs),
}

/// Arguments for `export gedcom`. The **whole database** is exported (GEDCOM is the
/// entire tree — there is no root/mode/generations, unlike `export html`), and living
/// persons are **not** redacted (a full-fidelity data move).
#[derive(Debug, Args)]
pub struct ExportGedcomArgs {
    /// Destination path for the `.ged` file.
    pub out: PathBuf,
    /// Overwrite `out` if it already exists.
    #[arg(long)]
    pub force: bool,
}

/// `import` subcommands. A sub-noun layer leaves room for other source formats
/// (CSV/JSON) later, mirroring `export html|gedcom`.
#[derive(Debug, Subcommand)]
pub enum ImportCommand {
    /// Import a GEDCOM 5.5.1 file.
    Gedcom(ImportGedcomArgs),
}

/// Arguments for `import gedcom`. Default imports into a **fresh** database (the
/// `--db` target is created + migrated if absent, and must be empty otherwise);
/// `--merge` appends into the existing database with fresh ids (additive — there is
/// **no de-duplication**).
#[derive(Debug, Args)]
pub struct ImportGedcomArgs {
    /// The GEDCOM file to read (UTF-8/ASCII; ANSEL/UTF-16 are rejected).
    pub file: PathBuf,
    /// Append into the existing database instead of requiring a fresh, empty one
    /// (additive — no de-duplication).
    #[arg(long)]
    pub merge: bool,
}

/// Arguments for `export html`. `out` is positional;
/// `--root` is a flag because `out` owns the positional slot. `--generations`
/// is range-checked by the core (over-budget → `Validation` → 4), not clap, so it
/// is a plain `u32` (mirroring [`QueryArgs`]).
#[derive(Debug, Args)]
pub struct ExportHtmlArgs {
    /// Destination path for the `.html` file.
    pub out: PathBuf,
    /// The focal individual to root the chart on.
    #[arg(long)]
    pub root: i64,
    /// Chart shape: `ancestors`, `descendants`, or `hourglass` (Network is GUI-only).
    #[arg(long, value_parser = parse_tree_mode)]
    pub mode: ChartMode,
    /// How many generations to walk from the root (counts edges, not nodes).
    #[arg(long, default_value_t = 4)]
    pub generations: u32,
    /// The palette the document opens in: `light` (default) or `dark`.
    #[arg(long, value_parser = parse_theme, default_value = "light")]
    pub theme: Theme,
    /// Include living individuals' details (off by default → they are redacted).
    #[arg(long)]
    pub include_living: bool,
    /// Embed each person's primary portrait (base64) in the export (off by
    /// default; living persons stay redacted unless `--include-living`).
    #[arg(long)]
    pub portraits: bool,
    /// Overwrite `out` if it already exists.
    #[arg(long)]
    pub force: bool,
}

/// `query` subcommands: the bounded relationship walks.
#[derive(Debug, Subcommand)]
pub enum QueryCommand {
    /// Walk a person's ancestors (parents, grandparents, …).
    Ancestors(QueryArgs),
    /// Walk a person's descendants (children, grandchildren, …).
    Descendants(QueryArgs),
}

/// Arguments shared by the `query` walks. `--generations` is range-checked by
/// the core walk (an over-budget value is a `Validation` error → exit 4), not by
/// clap, so it is a plain `u32` here.
#[derive(Debug, Args)]
pub struct QueryArgs {
    /// The focal individual's id.
    pub id: i64,
    /// How many generations to walk from the focus (counts edges, not nodes).
    #[arg(long, default_value_t = 4)]
    pub generations: u32,
}

/// `db` subcommands.
#[derive(Debug, Subcommand)]
pub enum DbCommand {
    /// Write a compacted backup to <FILE>.
    Backup(DbBackupArgs),
    /// Replace the current database with the backup at <FILE>.
    Restore(DbRestoreArgs),
    /// Rebuild the database in place, reclaiming free space.
    Vacuum,
}

/// Arguments for `db backup`.
#[derive(Debug, Args)]
pub struct DbBackupArgs {
    /// Destination path for the backup file.
    pub file: PathBuf,
    /// Overwrite the destination if it already exists.
    #[arg(long)]
    pub force: bool,
}

/// Arguments for `db restore`.
#[derive(Debug, Args)]
pub struct DbRestoreArgs {
    /// Backup file to restore from (validated before the target is touched).
    pub file: PathBuf,
    /// Overwrite the target database if it already exists.
    #[arg(long)]
    pub force: bool,
}

/// `person` subcommands.
#[derive(Debug, Subcommand)]
pub enum PersonCommand {
    /// Add a new individual.
    Add(PersonAddArgs),
    /// List individuals.
    List(PersonListArgs),
    /// Show one individual with their names, events, and family links.
    Show(PersonShowArgs),
    /// Edit an individual; only the flags you pass are changed.
    Edit(PersonEditArgs),
    /// Remove an individual (cascades to their names, memberships, and events).
    Rm(PersonRmArgs),
}

/// Arguments for `person add`. Each flag maps to one `NewIndividual` field.
#[derive(Debug, Args)]
pub struct PersonAddArgs {
    /// Primary given name(s).
    #[arg(long)]
    pub given: Option<String>,
    /// Primary surname.
    #[arg(long)]
    pub surname: Option<String>,
    /// Name prefix (e.g. "Dr").
    #[arg(long)]
    pub prefix: Option<String>,
    /// Name suffix (e.g. "Jr").
    #[arg(long)]
    pub suffix: Option<String>,
    /// Informal / known-as name.
    #[arg(long)]
    pub nickname: Option<String>,
    /// Recorded sex: one of `M`, `F`, `X`, `U`.
    #[arg(long, value_parser = parse_sex, default_value = "U")]
    pub sex: Sex,
    /// Whether the person is living (privacy/redaction flag).
    #[arg(long, value_name = "BOOL", default_value_t = true,
          action = clap::ArgAction::Set)]
    pub living: bool,
    /// Free-form notes.
    #[arg(long)]
    pub notes: Option<String>,
    /// Also record a birth event with this (fuzzy) date.
    #[arg(long, value_parser = parse_date, value_name = "DATE")]
    pub birth: Option<GenealogicalDate>,
    /// Also record a death event with this (fuzzy) date.
    #[arg(long, value_parser = parse_date, value_name = "DATE")]
    pub death: Option<GenealogicalDate>,
}

/// Arguments for `person list`.
#[derive(Debug, Args)]
pub struct PersonListArgs {
    /// Case-insensitive surname substring filter.
    #[arg(long)]
    pub surname: Option<String>,
}

/// Arguments for `person show`.
#[derive(Debug, Args)]
pub struct PersonShowArgs {
    /// The individual's id.
    pub id: i64,
}

/// Arguments for `person rm`.
#[derive(Debug, Args)]
pub struct PersonRmArgs {
    /// The individual's id.
    pub id: i64,
}

/// Arguments for `person edit`. Every field is optional: an absent flag leaves
/// the column unchanged. Clearing a field to NULL is not supported.
#[derive(Debug, Args)]
pub struct PersonEditArgs {
    /// The individual's id.
    pub id: i64,
    /// Primary given name(s).
    #[arg(long)]
    pub given: Option<String>,
    /// Primary surname.
    #[arg(long)]
    pub surname: Option<String>,
    /// Name prefix (e.g. "Dr").
    #[arg(long)]
    pub prefix: Option<String>,
    /// Name suffix (e.g. "Jr").
    #[arg(long)]
    pub suffix: Option<String>,
    /// Informal / known-as name.
    #[arg(long)]
    pub nickname: Option<String>,
    /// Recorded sex: one of `M`, `F`, `X`, `U`.
    #[arg(long, value_parser = parse_sex)]
    pub sex: Option<Sex>,
    /// Whether the person is living (privacy/redaction flag).
    #[arg(long, value_name = "BOOL", action = clap::ArgAction::Set)]
    pub living: Option<bool>,
    /// Free-form notes.
    #[arg(long)]
    pub notes: Option<String>,
}

/// `name` subcommands (an individual's alternate names).
#[derive(Debug, Subcommand)]
pub enum NameCommand {
    /// Attach an alternate name to an individual.
    Add(NameAddArgs),
    /// List an individual's alternate names.
    List(NameListArgs),
    /// Remove an alternate name by its id.
    Rm(NameRmArgs),
}

/// Arguments for `name add`.
#[derive(Debug, Args)]
pub struct NameAddArgs {
    /// The individual this name belongs to.
    pub person_id: i64,
    /// Kind of name: `birth`, `married`, `aka`, or `religious`.
    #[arg(long, value_parser = parse_name_kind)]
    pub kind: NameKind,
    /// Given name(s).
    #[arg(long)]
    pub given: Option<String>,
    /// Surname.
    #[arg(long)]
    pub surname: Option<String>,
    /// Name prefix.
    #[arg(long)]
    pub prefix: Option<String>,
    /// Name suffix.
    #[arg(long)]
    pub suffix: Option<String>,
    /// Display order among the individual's alternate names.
    #[arg(long, default_value_t = 0)]
    pub order: i64,
}

/// Arguments for `name list`.
#[derive(Debug, Args)]
pub struct NameListArgs {
    /// The individual whose names to list.
    pub person_id: i64,
}

/// Arguments for `name rm`.
#[derive(Debug, Args)]
pub struct NameRmArgs {
    /// The alternate name's id.
    pub id: i64,
}

/// `family` subcommands.
#[derive(Debug, Subcommand)]
pub enum FamilyCommand {
    /// Create a family/union with up to two partners.
    New(FamilyNewArgs),
    /// Add a partner to the family's first empty slot.
    AddPartner(FamilyAddPartnerArgs),
    /// Add a child to a family.
    AddChild(FamilyAddChildArgs),
    /// Show a family with its partners, children, and events.
    Show(FamilyShowArgs),
    /// List all families.
    List,
    /// Remove a family (cascades to memberships and family events).
    Rm(FamilyRmArgs),
    /// Remove a child's membership from a family.
    RemoveChild(FamilyRemoveChildArgs),
}

/// Arguments for `family new`.
#[derive(Debug, Args)]
pub struct FamilyNewArgs {
    /// A partner's id (repeatable; at most two).
    #[arg(long = "partner", value_name = "ID")]
    pub partner: Vec<i64>,
    /// The nature of the union: `marriage`, `partnership`, or `unknown`.
    #[arg(long = "type", value_parser = parse_union_type, default_value = "unknown")]
    pub union_type: UnionType,
    /// Free-form notes.
    #[arg(long)]
    pub notes: Option<String>,
}

/// Arguments for `family add-partner`.
#[derive(Debug, Args)]
pub struct FamilyAddPartnerArgs {
    /// The family's id.
    pub family_id: i64,
    /// The individual to add as a partner.
    pub person_id: i64,
}

/// Arguments for `family add-child`.
#[derive(Debug, Args)]
pub struct FamilyAddChildArgs {
    /// The family's id.
    pub family_id: i64,
    /// The individual to add as a child.
    pub person_id: i64,
    /// How the child is related: `birth`, `adopted`, `step`, or `foster`.
    #[arg(long, value_parser = parse_child_relation, default_value = "birth")]
    pub relation: ChildRelation,
    /// Birth order; defaults to appending after existing children.
    #[arg(long)]
    pub order: Option<i64>,
}

/// Arguments for `family show`.
#[derive(Debug, Args)]
pub struct FamilyShowArgs {
    /// The family's id.
    pub id: i64,
}

/// Arguments for `family rm`.
#[derive(Debug, Args)]
pub struct FamilyRmArgs {
    /// The family's id.
    pub id: i64,
}

/// Arguments for `family remove-child`.
#[derive(Debug, Args)]
pub struct FamilyRemoveChildArgs {
    /// The family's id.
    pub family_id: i64,
    /// The child to remove from the family.
    pub child_id: i64,
}

/// `event` subcommands.
#[derive(Debug, Subcommand)]
pub enum EventCommand {
    /// Add an event to an individual or a family.
    Add(EventAddArgs),
    /// Show one event with its resolved place.
    Show(EventShowArgs),
    /// Edit an event; only the flags you pass are changed (the subject is immutable).
    Edit(EventEditArgs),
    /// Remove an event.
    Rm(EventRmArgs),
}

/// Arguments for `event add`.
#[derive(Debug, Args)]
pub struct EventAddArgs {
    /// The subject this event is about: `person:<id>` or `family:<id>`.
    #[arg(long, value_parser = parse_subject)]
    pub subject: EventSubject,
    /// Kind of event: `birth`, `death`, `marriage`, `divorce`, `baptism`,
    /// `burial`, `residence`, `occupation`, or any other code (kept verbatim).
    #[arg(long, value_parser = parse_event_kind)]
    pub kind: EventKind,
    /// The (fuzzy) date, e.g. "12 Mar 1887", "ABT 1850", "BET 1900 AND 1910".
    #[arg(long, value_parser = parse_date)]
    pub date: Option<GenealogicalDate>,
    /// A new place name to attach (inserted each time; no dedup).
    #[arg(long, conflicts_with = "place_id")]
    pub place: Option<String>,
    /// An existing place id to attach (mutually exclusive with `--place`).
    #[arg(long = "place-id", value_name = "ID")]
    pub place_id: Option<i64>,
    /// Free-form notes.
    #[arg(long)]
    pub notes: Option<String>,
}

/// Arguments for `event show`.
#[derive(Debug, Args)]
pub struct EventShowArgs {
    /// The event's id.
    pub id: i64,
}

/// Arguments for `event edit`. Every field is optional: an absent flag leaves it
/// unchanged. The subject is immutable; clearing a field to NULL is not
/// supported (`--notes ""` stores an empty string, not NULL).
#[derive(Debug, Args)]
pub struct EventEditArgs {
    /// The event's id.
    pub id: i64,
    /// New event kind (any code; unknown kinds are kept verbatim).
    #[arg(long, value_parser = parse_event_kind)]
    pub kind: Option<EventKind>,
    /// New (fuzzy) date.
    #[arg(long, value_parser = parse_date)]
    pub date: Option<GenealogicalDate>,
    /// Replace the place with a new place name (inserted; no dedup).
    #[arg(long, conflicts_with = "place_id")]
    pub place: Option<String>,
    /// Replace the place with an existing place id.
    #[arg(long = "place-id", value_name = "ID")]
    pub place_id: Option<i64>,
    /// New notes.
    #[arg(long)]
    pub notes: Option<String>,
}

/// Arguments for `event rm`.
#[derive(Debug, Args)]
pub struct EventRmArgs {
    /// The event's id.
    pub id: i64,
}

/// clap value-parser for [`Sex`], delegating to its `FromStr`.
/// A bad code is a clap **usage** error → exit `2`.
fn parse_sex(s: &str) -> Result<Sex, String> {
    s.parse::<Sex>().map_err(|e| e.to_string())
}

/// clap value-parser for [`UnionType`] (delegates to its `FromStr`).
fn parse_union_type(s: &str) -> Result<UnionType, String> {
    s.parse::<UnionType>().map_err(|e| e.to_string())
}

/// clap value-parser for [`ChildRelation`] (delegates to its `FromStr`).
fn parse_child_relation(s: &str) -> Result<ChildRelation, String> {
    s.parse::<ChildRelation>().map_err(|e| e.to_string())
}

/// clap value-parser for [`NameKind`] (delegates to its `FromStr`).
fn parse_name_kind(s: &str) -> Result<NameKind, String> {
    s.parse::<NameKind>().map_err(|e| e.to_string())
}

/// clap value-parser for [`GenealogicalDate`] (delegates to its `FromStr`).
/// A malformed date is a clap **usage** error → exit `2`. Parsing at clap
/// time also satisfies the "validate before any write" guarantee.
fn parse_date(s: &str) -> Result<GenealogicalDate, String> {
    s.parse::<GenealogicalDate>().map_err(|e| e.to_string())
}

/// clap value-parser for an event's **open** [`EventKind`]: every string is
/// valid (an unknown code becomes [`EventKind::Other`]), so this never errors
/// — it returns `Result` only to satisfy clap's value-parser signature.
fn parse_event_kind(s: &str) -> Result<EventKind, String> {
    Ok(EventKind::from(s))
}

/// clap value-parser for an [`EventSubject`] from `person:<id>` / `family:<id>`.
/// A malformed subject is a clap **usage** error → exit `2`; a well-formed
/// but nonexistent id surfaces later as a foreign-key error → exit `6`.
fn parse_subject(s: &str) -> Result<EventSubject, String> {
    let (kind, id) = s
        .split_once(':')
        .ok_or_else(|| format!("expected `person:<id>` or `family:<id>`, got {s:?}"))?;
    let id: i64 = id
        .parse()
        .map_err(|_| format!("subject id must be an integer, got {id:?}"))?;
    match kind {
        "person" => Ok(EventSubject::Individual(PersonId::new(id))),
        "family" => Ok(EventSubject::Family(FamilyId::new(id))),
        other => Err(format!(
            "subject kind must be `person` or `family`, got {other:?}"
        )),
    }
}

/// clap value-parser for a [`MediaSubject`] from `person:<id>` / `family:<id>` /
/// `event:<id>` (media links also target events). A malformed subject
/// is a clap **usage** error → exit `2`.
fn parse_media_subject(s: &str) -> Result<MediaSubject, String> {
    let (kind, id) = s.split_once(':').ok_or_else(|| {
        format!("expected `person:<id>`, `family:<id>`, or `event:<id>`, got {s:?}")
    })?;
    let id: i64 = id
        .parse()
        .map_err(|_| format!("subject id must be an integer, got {id:?}"))?;
    match kind {
        "person" => Ok(MediaSubject::Individual(PersonId::new(id))),
        "family" => Ok(MediaSubject::Family(FamilyId::new(id))),
        "event" => Ok(MediaSubject::Event(EventId::new(id))),
        other => Err(format!(
            "subject kind must be `person`, `family`, or `event`, got {other:?}"
        )),
    }
}

/// clap value-parser for the **tree** chart modes. `network` and any other
/// string are clap usage errors → exit `2`. The core renders Network,
/// but it stays a GUI-only surface for now — the CLI `export html` offers the three
/// tree modes. Hand-mapped because [`ChartMode`] has no `FromStr`.
fn parse_tree_mode(s: &str) -> Result<ChartMode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "ancestors" => Ok(ChartMode::Ancestors),
        "descendants" => Ok(ChartMode::Descendants),
        "hourglass" => Ok(ChartMode::Hourglass),
        other => Err(format!(
            "mode must be `ancestors`, `descendants`, or `hourglass`, got {other:?}"
        )),
    }
}

/// clap value-parser for [`Theme`], delegating to its core `FromStr` — the
/// `parse_sex`/`parse_date` pattern. A bad value is a clap usage error → exit `2`.
fn parse_theme(s: &str) -> Result<Theme, String> {
    s.parse::<Theme>().map_err(|e| e.to_string())
}

/// clap value-parser for a [`CitationSubject`] from `event:<id>` / `person:<id>` /
/// `family:<id>`. A malformed subject is a clap **usage** error → exit
/// `2`. Mirrors [`parse_media_subject`].
fn parse_citation_subject(s: &str) -> Result<CitationSubject, String> {
    let (kind, id) = s.split_once(':').ok_or_else(|| {
        format!("expected `event:<id>`, `person:<id>`, or `family:<id>`, got {s:?}")
    })?;
    let id: i64 = id
        .parse()
        .map_err(|_| format!("subject id must be an integer, got {id:?}"))?;
    match kind {
        "person" => Ok(CitationSubject::Individual(PersonId::new(id))),
        "family" => Ok(CitationSubject::Family(FamilyId::new(id))),
        "event" => Ok(CitationSubject::Event(EventId::new(id))),
        other => Err(format!(
            "subject kind must be `person`, `family`, or `event`, got {other:?}"
        )),
    }
}

/// clap value-parser for [`Confidence`], delegating to its core `FromStr` (the TEXT
/// codes `primary`/`secondary`/`questionable`). A bad value is a usage error → `2`.
fn parse_confidence(s: &str) -> Result<Confidence, String> {
    s.parse::<Confidence>().map_err(|e| e.to_string())
}

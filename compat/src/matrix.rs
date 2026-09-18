//! The compatibility matrix: one row per machine nihilurk claims to run on.
//!
//! The table is `matrix.tsv`, and it is compiled in with `include_str!` rather
//! than read at runtime. That is the same decision as the content tables and
//! for the same reasons (see `docs/explanation/adr-0001-tables-not-raws.md`):
//! the binary is self-contained, a malformed row is found when the pipeline is
//! built rather than in the middle of a twenty-minute matrix run, and there is
//! no "where is the data file" question to answer on a machine that has only
//! the binary.
//!
//! The shell half of the pipeline reads the same file with `awk`. One table,
//! two readers, no second list to keep in step.

/// The raw table. Editing it is how a machine is added to the matrix.
const MATRIX: &str = include_str!("../matrix.tsv");

/// What kind of machine a row describes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    /// Has an operating system, a terminal and a shell. nihilurk runs here, and
    /// this is the only kind of row the stress matrix executes.
    Linux,
    /// No operating system. Only `particle-core` is built for it, and it is
    /// only ever checked, never run. See `nostd_check.sh`.
    Bare,
}

/// How the host gets the binary to execute.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Exec {
    /// The host CPU runs it directly. x86_64 on x86_64, and i686 on x86_64 --
    /// a 64-bit kernel runs 32-bit user space natively, so the "potato" row is
    /// not emulated either, only starved.
    Native,
    /// A directly-invoked qemu-user-static interpreter runs it -- no
    /// binfmt_misc, no `--privileged` (see `stress_test_matrix.sh`).
    /// Everything measured on such a row carries the emulator's tax; see
    /// `docs/explanation/cross-platform-testing.md`.
    Qemu,
    /// Never executed.
    None,
}

/// One machine.
#[derive(Clone, Debug)]
pub struct Row {
    pub id: String,
    pub target: String,
    pub class: Class,
    /// `docker --platform` value for a `Native` row. A `Qemu` row's
    /// container instead runs as the host's own architecture, so this only
    /// tells `stress_test_matrix.sh` which qemu-user-static interpreter to
    /// fetch.
    pub platform: String,
    pub image: String,
    pub exec: Exec,
    /// `docker --cpus`, as written in the table.
    pub cpus: String,
    /// `docker --memory`, as written in the table.
    pub memory: String,
    pub note: String,
}

impl Row {
    /// The container name a run of this row gets. Fixed rather than random so a
    /// run that was killed halfway can be found and removed by hand.
    pub fn container(&self) -> String {
        format!("nihilurk-compat-{}", self.id)
    }
}

/// Every row of the matrix, in file order.
///
/// Parsing cannot fail into an error type on purpose: a malformed line is
/// dropped, and a table that produced no rows at all is caught by the caller.
/// The alternative -- a `Result` threaded through every caller for a file that
/// is compiled in and reviewed with the code -- buys nothing.
pub fn rows() -> Vec<Row> {
    MATRIX
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim_start().starts_with('#') && !line.trim().is_empty())
        .filter_map(parse_row)
        .collect()
}

fn parse_row(line: &str) -> Option<Row> {
    let f: Vec<&str> = line.split('\t').collect();
    if f.len() < 9 {
        return None;
    }
    Some(Row {
        id: f[0].to_string(),
        target: f[1].to_string(),
        class: match f[2] {
            "bare" => Class::Bare,
            _ => Class::Linux,
        },
        platform: f[3].to_string(),
        image: f[4].to_string(),
        exec: match f[5] {
            "qemu" => Exec::Qemu,
            "none" => Exec::None,
            _ => Exec::Native,
        },
        cpus: f[6].to_string(),
        memory: f[7].to_string(),
        note: f[8].to_string(),
    })
}

/// The rows a run should cover: every Linux row, or the ones named in
/// `--targets`.
pub fn selected(only: &[String], class: Option<Class>) -> Vec<Row> {
    rows()
        .into_iter()
        .filter(|r| class.is_none_or(|c| r.class == c))
        .filter(|r| only.is_empty() || only.contains(&r.id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_parses_and_has_both_kinds_of_row() {
        let rows = rows();
        assert!(rows.len() >= 4, "matrix.tsv lost rows: {}", rows.len());
        assert!(rows.iter().any(|r| r.class == Class::Linux));
        assert!(rows.iter().any(|r| r.class == Class::Bare));
    }

    #[test]
    fn every_linux_row_is_a_musl_target() {
        // A -gnu row would build a binary bound to the glibc it was linked
        // against, which is the one thing the matrix exists to rule out.
        for row in rows().iter().filter(|r| r.class == Class::Linux) {
            assert!(
                row.target.contains("musl"),
                "{} is not a musl target: {}",
                row.id,
                row.target
            );
        }
    }

    #[test]
    fn bare_rows_are_never_executed() {
        for row in rows().iter().filter(|r| r.class == Class::Bare) {
            assert_eq!(row.exec, Exec::None, "{} would be executed", row.id);
        }
    }

    #[test]
    fn ids_are_unique_since_they_name_containers_and_files() {
        let mut ids: Vec<String> = rows().into_iter().map(|r| r.id).collect();
        ids.sort();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate id in matrix.tsv");
    }
}

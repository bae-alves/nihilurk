//! The compatibility matrix: one row per machine nihilurk claims to run on.
//!
//! The table is `matrix.tsv`, and it is compiled in with `include_str!` rather
//! than read at runtime. That is the same decision as the content tables and
//! for the same reasons (see `docs/explanation/adr-0001-tables-not-raws.md`):
//! the binary is self-contained, a malformed row is found when the pipeline is
//! built rather than in the middle of a matrix run, and there is no "where is
//! the data file" question to answer on a machine that has only the binary.
//!
//! The shell half of the pipeline reads the same file with `awk`. One table,
//! two readers, no second list to keep in step.

/// The raw table. Editing it is how a machine is added to the matrix.
const MATRIX: &str = include_str!("../matrix.tsv");

/// How the host gets the binary to execute.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Exec {
    /// The host CPU runs it directly. x86_64 on x86_64, and i686 on x86_64 --
    /// a 64-bit kernel runs 32-bit user space natively.
    Native,
    /// A directly-invoked qemu-user-static interpreter runs it -- no
    /// binfmt_misc, no `--privileged` (see `run_check.sh`).
    Qemu,
}

/// One machine.
#[derive(Clone, Debug)]
pub struct Row {
    pub id: String,
    pub target: String,
    /// `docker --platform` value for a `Native` row. A `Qemu` row's
    /// container instead runs as the host's own architecture, so this only
    /// tells `run_check.sh` which qemu-user-static interpreter to fetch.
    pub platform: String,
    pub image: String,
    pub exec: Exec,
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
    if f.len() < 7 {
        return None;
    }
    Some(Row {
        id: f[0].to_string(),
        target: f[1].to_string(),
        platform: f[3].to_string(),
        image: f[4].to_string(),
        exec: match f[5] {
            "qemu" => Exec::Qemu,
            _ => Exec::Native,
        },
    })
}

/// The rows a run should cover: every row, or the ones named in `--targets`.
pub fn selected(only: &[String]) -> Vec<Row> {
    rows()
        .into_iter()
        .filter(|r| only.is_empty() || only.contains(&r.id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_parses() {
        let rows = rows();
        assert!(rows.len() >= 4, "matrix.tsv lost rows: {}", rows.len());
    }

    #[test]
    fn every_linux_row_is_a_musl_target() {
        // A -gnu row would build a binary bound to the glibc it was linked
        // against, which is the one thing the matrix exists to rule out.
        for row in rows().iter() {
            assert!(
                row.target.contains("musl"),
                "{} is not a musl target: {}",
                row.id,
                row.target
            );
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

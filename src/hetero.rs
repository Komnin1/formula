//! Named ring systems for systematic names: heterocycles (pyridine, thiazole, piperazine…),
//! fused systems (benzo[d]thiazole, quinoline, imidazo[1,2-a]pyrazine, chromene…) and
//! naphthalene, each with its fixed IUPAC numbering.
//!
//! A drawn ring system is matched to a template by its atoms and ring bonds (bond orders are
//! ignored); every match is one allowed numbering. The namer then works out "hydro" prefixes,
//! indicated hydrogen ("2H-", "1H-") and added hydrogen ("-3(7H)-one") by comparing the drawn
//! double bonds with the template's fully unsaturated form.

/// A ring system with its numbering.
pub struct Template {
    /// The name of the parent ring system ("thiazole", "benzo[d]thiazole").
    pub name: &'static str,
    /// (element number, locant) for each atom, in numbering order.
    pub atoms: &'static [(u8, &'static str)],
    /// Ring bonds, as indices into `atoms`.
    pub bonds: &'static [(u8, u8)],
    /// How many atoms carry indicated hydrogen in the fully unsaturated form ("1H-indole": 1).
    pub indicated: usize,
    /// A saturated parent with a retained name (piperazine): no hydro prefixes.
    pub saturated: bool,
    /// Written before the name instead of indicated/hydro prefixes (boron–dipyrrole core).
    pub fixed_prefix: Option<&'static str>,
}

const C: u8 = 6;
const N: u8 = 7;
const O: u8 = 8;
const S: u8 = 16;
const B: u8 = 5;

const R5: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 0)];
const R6: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 0)];
/// Five-membered ring fused to a benzene ring: 1, 2, 3, 3a, 4, 5, 6, 7, 7a.
const F56: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 0), (8, 3)];
/// Two fused six-membered rings: 1, 2, 3, 4, 4a, 5, 6, 7, 8, 8a.
const F66: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 0), (9, 4)];
/// Five- and six-membered rings sharing a nitrogen bridgehead: 1, 2, 3, 4, 5, 6, 7, 8, 8a.
const F56N: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 0), (8, 3)];

const R3: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 0)];
const R4: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 0)];
const R7: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 0)];
/// Six-membered rings sharing a nitrogen bridgehead (quinolizine): 1, 2, 3, 4, 5, 6, 7, 8, 9, 9a.
const Q66N: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 0), (4, 9)];
/// Two five-membered rings: 1, 2, 3, 3a (or 4), 4…, 6a/7a.
const F55: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 0), (3, 7)];
/// Five- and seven-membered rings (azulene): 1, 2, 3, 3a, 4…8, 8a.
const F57: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 0), (3, 9)];
/// Seven-membered ring numbered first, fused to benzene: 1…5, 5a, 6…9, 9a.
const F67: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 10), (10, 0), (5, 10)];
/// Benzene numbered first, fused to a seven-membered ring: 1…4, 4a, 5…9, 9a.
const B7A: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 10), (10, 0), (4, 10)];
/// Anthracene numbering: 1, 2, 3, 4, 4a, 5, 6, 7, 8, 8a, 9, 9a, 10, 10a.
const F666L: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 12), (12, 13), (13, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 10), (10, 11), (11, 0), (4, 11), (9, 13)];
/// Linear three rings numbered round the edge (phenothiazine): 1…4, 4a, 5, 5a, 6…9, 9a, 10, 10a.
const F666P: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 10), (10, 11), (11, 12), (12, 13), (13, 0), (4, 13), (6, 11)];
/// Phenanthrene: 1…4, 4a, 4b, 5…8, 8a, 9, 10, 10a.
const F666A: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 10), (10, 11), (11, 12), (12, 13), (13, 0), (4, 13), (5, 10)];
/// Phenanthridine and phenanthroline: 1…4, 4a, 5, 6, 6a, 7…10, 10a, 10b.
const F666B: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 10), (10, 11), (11, 12), (12, 13), (13, 0), (4, 13), (7, 12)];
/// Carbazole and fluorene: 1…4, 4a, 4b, 5…8, 8a, 9, 9a.
const F656: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 10), (10, 11), (11, 12), (12, 0), (4, 12), (5, 10)];
/// Dibenzofuran: 1…4, 4a, 5, 5a, 6…9, 9a, 9b.
const F656H: &[(u8, u8)] = &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 10), (10, 11), (11, 12), (12, 0), (4, 12), (6, 11)];
/// The steroid skeleton, atoms 1…17 as steroids are numbered.
const STEROID: &[(u8, &str)] = &[
    (C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "9"), (C, "10"), (C, "11"), (C, "12"), (C, "13"), (C, "14"), (C, "15"), (C, "16"), (C, "17"),
];
const STEROID_BONDS: &[(u8, u8)] = &[
    (0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 0), (4, 9), (8, 10), (10, 11), (11, 12), (12, 13), (13, 7), (12, 16), (16, 15), (15, 14), (14, 13),
];
const PYRENE_BONDS: &[(u8, u8)] = &[
    (0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 10), (10, 11), (11, 12), (12, 13), (13, 0), (3, 14), (13, 14), (14, 15), (6, 15), (10, 15),
];
const CHRYSENE_BONDS: &[(u8, u8)] = &[
    (0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 10), (10, 11), (11, 12), (12, 13), (13, 14), (14, 15), (15, 16), (16, 17), (17, 0), (4, 17), (5, 14), (8, 13),
];
const TETRACENE_BONDS: &[(u8, u8)] = &[
    (0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 10), (10, 11), (11, 12), (12, 13), (13, 14), (14, 15), (15, 16), (16, 17), (17, 0), (4, 17), (6, 15), (8, 13),
];

const L5: [&str; 5] = ["1", "2", "3", "4", "5"];
const L6: [&str; 6] = ["1", "2", "3", "4", "5", "6"];

macro_rules! t {
    ($name:expr, $atoms:expr, $bonds:expr, $ind:expr) => {
        Template { name: $name, atoms: $atoms, bonds: $bonds, indicated: $ind, saturated: false, fixed_prefix: None }
    };
    ($name:expr, $atoms:expr, $bonds:expr, $ind:expr, sat) => {
        Template { name: $name, atoms: $atoms, bonds: $bonds, indicated: $ind, saturated: true, fixed_prefix: None }
    };
}

/// Saturated retained names come before the unsaturated parent with the same skeleton.
pub static TEMPLATES: &[Template] = &[
    // Saturated five-membered rings
    t!("pyrrolidine", &[(N, L5[0]), (C, L5[1]), (C, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0, sat),
    t!("tetrahydrofuran", &[(O, L5[0]), (C, L5[1]), (C, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0, sat),
    t!("tetrahydrothiophene", &[(S, L5[0]), (C, L5[1]), (C, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0, sat),
    t!("imidazolidine", &[(N, L5[0]), (C, L5[1]), (N, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0, sat),
    t!("pyrazolidine", &[(N, L5[0]), (N, L5[1]), (C, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0, sat),
    t!("oxazolidine", &[(O, L5[0]), (C, L5[1]), (N, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0, sat),
    t!("thiazolidine", &[(S, L5[0]), (C, L5[1]), (N, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0, sat),
    t!("1,3-dioxolane", &[(O, L5[0]), (C, L5[1]), (O, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0, sat),
    // Five-membered heteroaromatics
    t!("pyrrole", &[(N, L5[0]), (C, L5[1]), (C, L5[2]), (C, L5[3]), (C, L5[4])], R5, 1),
    t!("furan", &[(O, L5[0]), (C, L5[1]), (C, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0),
    t!("thiophene", &[(S, L5[0]), (C, L5[1]), (C, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0),
    t!("imidazole", &[(N, L5[0]), (C, L5[1]), (N, L5[2]), (C, L5[3]), (C, L5[4])], R5, 1),
    t!("pyrazole", &[(N, L5[0]), (N, L5[1]), (C, L5[2]), (C, L5[3]), (C, L5[4])], R5, 1),
    t!("oxazole", &[(O, L5[0]), (C, L5[1]), (N, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0),
    t!("isoxazole", &[(O, L5[0]), (N, L5[1]), (C, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0),
    t!("thiazole", &[(S, L5[0]), (C, L5[1]), (N, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0),
    t!("isothiazole", &[(S, L5[0]), (N, L5[1]), (C, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0),
    t!("1,2,3-triazole", &[(N, L5[0]), (N, L5[1]), (N, L5[2]), (C, L5[3]), (C, L5[4])], R5, 1),
    t!("1,2,4-triazole", &[(N, L5[0]), (N, L5[1]), (C, L5[2]), (N, L5[3]), (C, L5[4])], R5, 1),
    t!("tetrazole", &[(N, L5[0]), (N, L5[1]), (N, L5[2]), (N, L5[3]), (C, L5[4])], R5, 1),
    t!("1,3,4-oxadiazole", &[(O, L5[0]), (C, L5[1]), (N, L5[2]), (N, L5[3]), (C, L5[4])], R5, 0),
    t!("1,2,4-oxadiazole", &[(O, L5[0]), (N, L5[1]), (C, L5[2]), (N, L5[3]), (C, L5[4])], R5, 0),
    t!("1,3,4-thiadiazole", &[(S, L5[0]), (C, L5[1]), (N, L5[2]), (N, L5[3]), (C, L5[4])], R5, 0),
    // Saturated six-membered rings
    t!("piperidine", &[(N, L6[0]), (C, L6[1]), (C, L6[2]), (C, L6[3]), (C, L6[4]), (C, L6[5])], R6, 0, sat),
    t!("piperazine", &[(N, L6[0]), (C, L6[1]), (C, L6[2]), (N, L6[3]), (C, L6[4]), (C, L6[5])], R6, 0, sat),
    t!("morpholine", &[(O, L6[0]), (C, L6[1]), (C, L6[2]), (N, L6[3]), (C, L6[4]), (C, L6[5])], R6, 0, sat),
    t!("thiomorpholine", &[(S, L6[0]), (C, L6[1]), (C, L6[2]), (N, L6[3]), (C, L6[4]), (C, L6[5])], R6, 0, sat),
    t!("1,4-dioxane", &[(O, L6[0]), (C, L6[1]), (C, L6[2]), (O, L6[3]), (C, L6[4]), (C, L6[5])], R6, 0, sat),
    // Six-membered heteroaromatics
    t!("pyridine", &[(N, L6[0]), (C, L6[1]), (C, L6[2]), (C, L6[3]), (C, L6[4]), (C, L6[5])], R6, 0),
    t!("pyridazine", &[(N, L6[0]), (N, L6[1]), (C, L6[2]), (C, L6[3]), (C, L6[4]), (C, L6[5])], R6, 0),
    t!("pyrimidine", &[(N, L6[0]), (C, L6[1]), (N, L6[2]), (C, L6[3]), (C, L6[4]), (C, L6[5])], R6, 0),
    t!("pyrazine", &[(N, L6[0]), (C, L6[1]), (C, L6[2]), (N, L6[3]), (C, L6[4]), (C, L6[5])], R6, 0),
    t!("1,3,5-triazine", &[(N, L6[0]), (C, L6[1]), (N, L6[2]), (C, L6[3]), (N, L6[4]), (C, L6[5])], R6, 0),
    t!("pyran", &[(O, L6[0]), (C, L6[1]), (C, L6[2]), (C, L6[3]), (C, L6[4]), (C, L6[5])], R6, 1),
    t!("thiopyran", &[(S, L6[0]), (C, L6[1]), (C, L6[2]), (C, L6[3]), (C, L6[4]), (C, L6[5])], R6, 1),
    // Benzene fused to a five-membered ring
    t!("indole", &[(N, "1"), (C, "2"), (C, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F56, 1),
    t!("benzofuran", &[(O, "1"), (C, "2"), (C, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F56, 0),
    t!("benzo[b]thiophene", &[(S, "1"), (C, "2"), (C, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F56, 0),
    t!("benzo[d]imidazole", &[(N, "1"), (C, "2"), (N, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F56, 1),
    t!("benzo[d]oxazole", &[(O, "1"), (C, "2"), (N, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F56, 0),
    t!("benzo[d]thiazole", &[(S, "1"), (C, "2"), (N, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F56, 0),
    t!("indazole", &[(N, "1"), (N, "2"), (C, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F56, 1),
    t!("indene", &[(C, "1"), (C, "2"), (C, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F56, 1),
    // Two six-membered rings
    t!("naphthalene", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F66, 0),
    t!("quinoline", &[(N, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F66, 0),
    t!("isoquinoline", &[(C, "1"), (N, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F66, 0),
    t!("quinazoline", &[(N, "1"), (C, "2"), (N, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F66, 0),
    t!("quinoxaline", &[(N, "1"), (C, "2"), (C, "3"), (N, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F66, 0),
    t!("cinnoline", &[(N, "1"), (N, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F66, 0),
    t!("phthalazine", &[(C, "1"), (N, "2"), (N, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F66, 0),
    t!("chromene", &[(O, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F66, 1),
    t!("isochromene", &[(C, "1"), (O, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F66, 1),
    t!("thiochromene", &[(S, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F66, 1),
    // Five- and six-membered rings sharing a nitrogen
    t!("imidazo[1,2-a]pyrazine", &[(N, "1"), (C, "2"), (C, "3"), (N, "4"), (C, "5"), (C, "6"), (N, "7"), (C, "8"), (C, "8a")], F56N, 0),
    t!("imidazo[1,2-a]pyridine", &[(N, "1"), (C, "2"), (C, "3"), (N, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F56N, 0),
    t!("imidazo[1,2-a]pyrimidine", &[(N, "1"), (C, "2"), (C, "3"), (N, "4"), (C, "5"), (C, "6"), (N, "7"), (C, "8"), (C, "8a")], F56N, 0),
    t!("indolizine", &[(C, "1"), (C, "2"), (C, "3"), (N, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F56N, 0),
    // Purine has its own numbering.
    t!(
        "purine",
        &[(N, "1"), (C, "2"), (N, "3"), (C, "4"), (C, "5"), (C, "6"), (N, "7"), (C, "8"), (N, "9")],
        &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 0), (4, 6), (6, 7), (7, 8), (8, 3)],
        1
    ),
    // Three- and four-membered rings
    t!("oxirane", &[(O, "1"), (C, "2"), (C, "3")], R3, 0, sat),
    t!("aziridine", &[(N, "1"), (C, "2"), (C, "3")], R3, 0, sat),
    t!("thiirane", &[(S, "1"), (C, "2"), (C, "3")], R3, 0, sat),
    t!("oxetane", &[(O, "1"), (C, "2"), (C, "3"), (C, "4")], R4, 0, sat),
    t!("azetidine", &[(N, "1"), (C, "2"), (C, "3"), (C, "4")], R4, 0, sat),
    t!("thietane", &[(S, "1"), (C, "2"), (C, "3"), (C, "4")], R4, 0, sat),
    t!("1,3-dioxane", &[(O, L6[0]), (C, L6[1]), (O, L6[2]), (C, L6[3]), (C, L6[4]), (C, L6[5])], R6, 0, sat),
    t!("1,3-dithiane", &[(S, L6[0]), (C, L6[1]), (S, L6[2]), (C, L6[3]), (C, L6[4]), (C, L6[5])], R6, 0, sat),
    t!("1,3-dithiolane", &[(S, L5[0]), (C, L5[1]), (S, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0, sat),
    t!("1,3-oxathiolane", &[(O, L5[0]), (C, L5[1]), (S, L5[2]), (C, L5[3]), (C, L5[4])], R5, 0, sat),
    // Seven-membered rings
    t!("azepane", &[(N, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "5"), (C, "6"), (C, "7")], R7, 0, sat),
    t!("oxepane", &[(O, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "5"), (C, "6"), (C, "7")], R7, 0, sat),
    t!("1,4-diazepane", &[(N, "1"), (C, "2"), (C, "3"), (N, "4"), (C, "5"), (C, "6"), (C, "7")], R7, 0, sat),
    t!("azepine", &[(N, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "5"), (C, "6"), (C, "7")], R7, 1),
    t!("oxepine", &[(O, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "5"), (C, "6"), (C, "7")], R7, 0),
    t!("1,4-diazepine", &[(N, "1"), (C, "2"), (C, "3"), (N, "4"), (C, "5"), (C, "6"), (C, "7")], R7, 1),
    // More six-six systems
    t!("1,8-naphthyridine", &[(N, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (N, "8"), (C, "8a")], F66, 0),
    t!("1,5-naphthyridine", &[(N, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (N, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F66, 0),
    t!("1,6-naphthyridine", &[(N, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (N, "6"), (C, "7"), (C, "8"), (C, "8a")], F66, 0),
    t!("1,7-naphthyridine", &[(N, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (N, "7"), (C, "8"), (C, "8a")], F66, 0),
    t!("pteridine", &[(N, "1"), (C, "2"), (N, "3"), (C, "4"), (C, "4a"), (N, "5"), (C, "6"), (C, "7"), (N, "8"), (C, "8a")], F66, 0),
    t!("pyrido[2,3-d]pyrimidine", &[(N, "1"), (C, "2"), (N, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (N, "8"), (C, "8a")], F66, 0),
    t!("benzo[b][1,4]oxazine", &[(O, "1"), (C, "2"), (C, "3"), (N, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F66, 0),
    t!("benzo[b][1,4]dioxine", &[(O, "1"), (C, "2"), (C, "3"), (O, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F66, 0),
    t!("quinolizine", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (N, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "9"), (C, "9a")], Q66N, 1),
    // Five-membered rings fused to other rings
    t!("benzo[d][1,3]dioxole", &[(O, "1"), (C, "2"), (O, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F56, 0),
    t!("isoindole", &[(C, "1"), (N, "2"), (C, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F56, 1),
    t!("isobenzofuran", &[(C, "1"), (O, "2"), (C, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F56, 0),
    t!("benzo[d]isoxazole", &[(O, "1"), (N, "2"), (C, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F56, 0),
    t!("benzo[d]isothiazole", &[(S, "1"), (N, "2"), (C, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F56, 0),
    t!("benzo[d][1,2,3]triazole", &[(N, "1"), (N, "2"), (N, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F56, 1),
    t!("pyrrolo[2,3-b]pyridine", &[(N, "1"), (C, "2"), (C, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (N, "7"), (C, "7a")], F56, 1),
    t!("pyrazolo[3,4-d]pyrimidine", &[(N, "1"), (N, "2"), (C, "3"), (C, "3a"), (C, "4"), (N, "5"), (C, "6"), (N, "7"), (C, "7a")], F56, 1),
    t!("pyrrolizine", &[(C, "1"), (C, "2"), (C, "3"), (N, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "7a")], F55, 1),
    t!("thieno[3,4-d]imidazole", &[(N, "1"), (C, "2"), (N, "3"), (C, "3a"), (C, "4"), (S, "5"), (C, "6"), (C, "6a")], F55, 1),
    t!("azulene", &[(C, "1"), (C, "2"), (C, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a")], F57, 0),
    t!("pentalene", &[(C, "1"), (C, "2"), (C, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "6"), (C, "6a")], F55, 0),
    // Benzene fused to a seven-membered ring
    t!("benzo[e][1,4]diazepine", &[(N, "1"), (C, "2"), (C, "3"), (N, "4"), (C, "5"), (C, "5a"), (C, "6"), (C, "7"), (C, "8"), (C, "9"), (C, "9a")], F67, 1),
    t!("benzo[b]azepine", &[(N, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "5"), (C, "5a"), (C, "6"), (C, "7"), (C, "8"), (C, "9"), (C, "9a")], F67, 1),
    t!("benzo[7]annulene", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "9"), (C, "9a")], B7A, 1),
    // Three rings in a row (anthracene numbering) and angular (phenanthrene numbering)
    t!("anthracene", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a"), (C, "9"), (C, "9a"), (C, "10"), (C, "10a")], F666L, 0),
    t!("acridine", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a"), (C, "9"), (C, "9a"), (N, "10"), (C, "10a")], F666L, 0),
    t!("xanthene", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a"), (C, "9"), (C, "9a"), (O, "10"), (C, "10a")], F666L, 1),
    t!("thioxanthene", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a"), (C, "9"), (C, "9a"), (S, "10"), (C, "10a")], F666L, 1),
    t!("phenothiazine", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (S, "5"), (C, "5a"), (C, "6"), (C, "7"), (C, "8"), (C, "9"), (C, "9a"), (N, "10"), (C, "10a")], F666P, 1),
    t!("phenoxazine", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (O, "5"), (C, "5a"), (C, "6"), (C, "7"), (C, "8"), (C, "9"), (C, "9a"), (N, "10"), (C, "10a")], F666P, 1),
    t!("phenazine", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (N, "5"), (C, "5a"), (C, "6"), (C, "7"), (C, "8"), (C, "9"), (C, "9a"), (N, "10"), (C, "10a")], F666P, 0),
    t!("dibenzo[b,e][1,4]dioxine", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (O, "5"), (C, "5a"), (C, "6"), (C, "7"), (C, "8"), (C, "9"), (C, "9a"), (O, "10"), (C, "10a")], F666P, 0),
    t!("thianthrene", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (S, "5"), (C, "5a"), (C, "6"), (C, "7"), (C, "8"), (C, "9"), (C, "9a"), (S, "10"), (C, "10a")], F666P, 0),
    t!("phenanthrene", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "4b"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a"), (C, "9"), (C, "10"), (C, "10a")], F666A, 0),
    t!("phenanthridine", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (N, "5"), (C, "6"), (C, "6a"), (C, "7"), (C, "8"), (C, "9"), (C, "10"), (C, "10a"), (C, "10b")], F666B, 0),
    t!("1,10-phenanthroline", &[(N, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "6"), (C, "6a"), (C, "7"), (C, "8"), (C, "9"), (N, "10"), (C, "10a"), (C, "10b")], F666B, 0),
    // Two benzene rings on a five-membered ring
    t!("carbazole", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "4b"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a"), (N, "9"), (C, "9a")], F656, 1),
    t!("fluorene", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "4b"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a"), (C, "9"), (C, "9a")], F656, 1),
    t!("dibenzo[b,d]furan", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (O, "5"), (C, "5a"), (C, "6"), (C, "7"), (C, "8"), (C, "9"), (C, "9a"), (C, "9b")], F656H, 0),
    t!("dibenzo[b,d]thiophene", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (S, "5"), (C, "5a"), (C, "6"), (C, "7"), (C, "8"), (C, "9"), (C, "9a"), (C, "9b")], F656H, 0),
    t!("pyrido[3,4-b]indole", &[(C, "1"), (N, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "4b"), (C, "5"), (C, "6"), (C, "7"), (C, "8"), (C, "8a"), (N, "9"), (C, "9a")], F656, 1),
    // Four rings: the steroid skeleton (numbered as steroids are) and pyrene, tetracene
    t!("cyclopenta[a]phenanthrene", STEROID, STEROID_BONDS, 1),
    t!("pyrene", &[(C, "1"), (C, "2"), (C, "3"), (C, "3a"), (C, "4"), (C, "5"), (C, "5a"), (C, "6"), (C, "7"), (C, "8"), (C, "8a"), (C, "9"), (C, "10"), (C, "10a"), (C, "10b"), (C, "10c")], PYRENE_BONDS, 0),
    t!("chrysene", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "4b"), (C, "5"), (C, "6"), (C, "6a"), (C, "7"), (C, "8"), (C, "9"), (C, "10"), (C, "10a"), (C, "10b"), (C, "11"), (C, "12"), (C, "12a")], CHRYSENE_BONDS, 0),
    t!("tetracene", &[(C, "1"), (C, "2"), (C, "3"), (C, "4"), (C, "4a"), (C, "5"), (C, "5a"), (C, "6"), (C, "6a"), (C, "7"), (C, "8"), (C, "9"), (C, "10"), (C, "10a"), (C, "11"), (C, "11a"), (C, "12"), (C, "12a")], TETRACENE_BONDS, 0),
    // The boron-dipyrromethene (BODIPY) core.
    Template {
        name: "dipyrrolo[1,2-c:2',1'-f][1,3,2]diazaborinine",
        atoms: &[(C, "1"), (C, "2"), (C, "3"), (N, "4"), (B, "5"), (N, "6"), (C, "7"), (C, "8"), (C, "9"), (C, "9a"), (C, "10"), (C, "10a")],
        bonds: &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 5), (9, 10), (10, 11), (11, 3), (11, 0)],
        indicated: 0,
        saturated: false,
        fixed_prefix: Some("5H-4λ4,5λ4-"),
    },
];

/// Sort key for a locant: "3" < "3a" < "4".
pub fn locant_key(l: &str) -> u32 {
    let digits: String = l.chars().take_while(|c| c.is_ascii_digit()).collect();
    let n: u32 = digits.parse().unwrap_or(0);
    let letter = l.chars().find(|c| c.is_ascii_alphabetic()).map(|c| c as u32 - 'a' as u32 + 1).unwrap_or(0);
    n * 32 + letter
}

/// The form used before "-yl" and vowel suffixes: "thiazole" → "thiazol", "furan" stays.
pub fn stem(name: &str) -> &str {
    name.strip_suffix('e').unwrap_or(name)
}

/// Match a drawn ring system to a template. `atoms`: the system's atoms; `el(i)`: element;
/// `ring_nbs(i)`: neighbours of i inside the system. Returns the template index and every
/// numbering (`map[t]` = the drawn atom for template atom t).
pub fn match_system(atoms: &[usize], el: &dyn Fn(usize) -> u8, ring_nbs: &dyn Fn(usize) -> Vec<usize>, saturated: bool) -> Option<(usize, Vec<Vec<usize>>)> {
    let n = atoms.len();
    let ring_bonds: usize = atoms.iter().map(|&a| ring_nbs(a).len()).sum::<usize>() / 2;
    for (ti, t) in TEMPLATES.iter().enumerate() {
        if t.atoms.len() != n || t.bonds.len() != ring_bonds || (t.saturated && !saturated) {
            continue;
        }
        let mut tadj = vec![Vec::new(); n];
        for &(a, b) in t.bonds {
            tadj[a as usize].push(b as usize);
            tadj[b as usize].push(a as usize);
        }
        let mut maps = Vec::new();
        let mut map = vec![usize::MAX; n];
        let mut used = std::collections::HashSet::new();
        extend(0, t, &tadj, atoms, el, ring_nbs, &mut map, &mut used, &mut maps);
        if !maps.is_empty() {
            return Some((ti, maps));
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn extend(
    k: usize,
    t: &Template,
    tadj: &[Vec<usize>],
    atoms: &[usize],
    el: &dyn Fn(usize) -> u8,
    ring_nbs: &dyn Fn(usize) -> Vec<usize>,
    map: &mut Vec<usize>,
    used: &mut std::collections::HashSet<usize>,
    out: &mut Vec<Vec<usize>>,
) {
    if out.len() >= 48 {
        return;
    }
    if k == t.atoms.len() {
        out.push(map.clone());
        return;
    }
    for &a in atoms {
        if used.contains(&a) || el(a) != t.atoms[k].0 || ring_nbs(a).len() != tadj[k].len() {
            continue;
        }
        // Bonds to template atoms already placed must exist in the drawing.
        let nbs = ring_nbs(a);
        if tadj[k].iter().any(|&j| j < k && !nbs.contains(&map[j])) {
            continue;
        }
        map[k] = a;
        used.insert(a);
        extend(k + 1, t, tadj, atoms, el, ring_nbs, map, used, out);
        used.remove(&a);
        map[k] = usize::MAX;
    }
}

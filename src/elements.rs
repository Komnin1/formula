//! Periodic table data: symbols, names, masses, valences and isotopes.

pub struct Element {
    pub sym: &'static str,
    pub name: &'static str,
    /// Standard atomic weight.
    pub mass: f64,
    /// Mass of the most abundant isotope.
    pub mono: f64,
}

macro_rules! el {
    ($s:expr, $n:expr, $m:expr, $mono:expr) => {
        Element { sym: $s, name: $n, mass: $m, mono: $mono }
    };
}

/// Index 0 is a placeholder (unknown / pseudo atom).
pub static ELEMENTS: [Element; 119] = [
    el!("*", "Unknown", 0.0, 0.0),
    el!("H", "Hydrogen", 1.008, 1.007825),
    el!("He", "Helium", 4.0026, 4.002603),
    el!("Li", "Lithium", 6.94, 7.016004),
    el!("Be", "Beryllium", 9.0122, 9.012182),
    el!("B", "Boron", 10.81, 11.009305),
    el!("C", "Carbon", 12.011, 12.0),
    el!("N", "Nitrogen", 14.007, 14.003074),
    el!("O", "Oxygen", 15.999, 15.994915),
    el!("F", "Fluorine", 18.998, 18.998403),
    el!("Ne", "Neon", 20.180, 19.992440),
    el!("Na", "Sodium", 22.990, 22.989770),
    el!("Mg", "Magnesium", 24.305, 23.985042),
    el!("Al", "Aluminium", 26.982, 26.981538),
    el!("Si", "Silicon", 28.085, 27.976927),
    el!("P", "Phosphorus", 30.974, 30.973762),
    el!("S", "Sulfur", 32.06, 31.972071),
    el!("Cl", "Chlorine", 35.45, 34.968853),
    el!("Ar", "Argon", 39.948, 39.962383),
    el!("K", "Potassium", 39.098, 38.963707),
    el!("Ca", "Calcium", 40.078, 39.962591),
    el!("Sc", "Scandium", 44.956, 44.955910),
    el!("Ti", "Titanium", 47.867, 47.947947),
    el!("V", "Vanadium", 50.942, 50.943964),
    el!("Cr", "Chromium", 51.996, 51.940512),
    el!("Mn", "Manganese", 54.938, 54.938050),
    el!("Fe", "Iron", 55.845, 55.934942),
    el!("Co", "Cobalt", 58.933, 58.933200),
    el!("Ni", "Nickel", 58.693, 57.935348),
    el!("Cu", "Copper", 63.546, 62.929601),
    el!("Zn", "Zinc", 65.38, 63.929147),
    el!("Ga", "Gallium", 69.723, 68.925581),
    el!("Ge", "Germanium", 72.630, 73.921178),
    el!("As", "Arsenic", 74.922, 74.921596),
    el!("Se", "Selenium", 78.971, 79.916522),
    el!("Br", "Bromine", 79.904, 78.918338),
    el!("Kr", "Krypton", 83.798, 83.911507),
    el!("Rb", "Rubidium", 85.468, 84.911789),
    el!("Sr", "Strontium", 87.62, 87.905614),
    el!("Y", "Yttrium", 88.906, 88.905848),
    el!("Zr", "Zirconium", 91.224, 89.904704),
    el!("Nb", "Niobium", 92.906, 92.906378),
    el!("Mo", "Molybdenum", 95.95, 97.905408),
    el!("Tc", "Technetium", 98.0, 97.907216),
    el!("Ru", "Ruthenium", 101.07, 101.904350),
    el!("Rh", "Rhodium", 102.91, 102.905504),
    el!("Pd", "Palladium", 106.42, 105.903483),
    el!("Ag", "Silver", 107.87, 106.905093),
    el!("Cd", "Cadmium", 112.41, 113.903358),
    el!("In", "Indium", 114.82, 114.903878),
    el!("Sn", "Tin", 118.71, 119.902197),
    el!("Sb", "Antimony", 121.76, 120.903818),
    el!("Te", "Tellurium", 127.60, 129.906223),
    el!("I", "Iodine", 126.90, 126.904468),
    el!("Xe", "Xenon", 131.29, 131.904154),
    el!("Cs", "Caesium", 132.91, 132.905447),
    el!("Ba", "Barium", 137.33, 137.905241),
    el!("La", "Lanthanum", 138.91, 138.906348),
    el!("Ce", "Cerium", 140.12, 139.905434),
    el!("Pr", "Praseodymium", 140.91, 140.907648),
    el!("Nd", "Neodymium", 144.24, 141.907719),
    el!("Pm", "Promethium", 145.0, 144.912744),
    el!("Sm", "Samarium", 150.36, 151.919728),
    el!("Eu", "Europium", 151.96, 152.921226),
    el!("Gd", "Gadolinium", 157.25, 157.924101),
    el!("Tb", "Terbium", 158.93, 158.925343),
    el!("Dy", "Dysprosium", 162.50, 163.929171),
    el!("Ho", "Holmium", 164.93, 164.930319),
    el!("Er", "Erbium", 167.26, 165.930290),
    el!("Tm", "Thulium", 168.93, 168.934211),
    el!("Yb", "Ytterbium", 173.05, 173.938858),
    el!("Lu", "Lutetium", 174.97, 174.940768),
    el!("Hf", "Hafnium", 178.49, 179.946549),
    el!("Ta", "Tantalum", 180.95, 180.947996),
    el!("W", "Tungsten", 183.84, 183.950933),
    el!("Re", "Rhenium", 186.21, 186.955751),
    el!("Os", "Osmium", 190.23, 191.961479),
    el!("Ir", "Iridium", 192.22, 192.962924),
    el!("Pt", "Platinum", 195.08, 194.964774),
    el!("Au", "Gold", 196.97, 196.966552),
    el!("Hg", "Mercury", 200.59, 201.970626),
    el!("Tl", "Thallium", 204.38, 204.974412),
    el!("Pb", "Lead", 207.2, 207.976636),
    el!("Bi", "Bismuth", 208.98, 208.980383),
    el!("Po", "Polonium", 209.0, 208.982416),
    el!("At", "Astatine", 210.0, 209.987131),
    el!("Rn", "Radon", 222.0, 222.017570),
    el!("Fr", "Francium", 223.0, 223.019731),
    el!("Ra", "Radium", 226.0, 226.025403),
    el!("Ac", "Actinium", 227.0, 227.027747),
    el!("Th", "Thorium", 232.04, 232.038050),
    el!("Pa", "Protactinium", 231.04, 231.035879),
    el!("U", "Uranium", 238.03, 238.050783),
    el!("Np", "Neptunium", 237.0, 237.048167),
    el!("Pu", "Plutonium", 244.0, 244.064198),
    el!("Am", "Americium", 243.0, 243.061373),
    el!("Cm", "Curium", 247.0, 247.070347),
    el!("Bk", "Berkelium", 247.0, 247.070299),
    el!("Cf", "Californium", 251.0, 251.079580),
    el!("Es", "Einsteinium", 252.0, 252.082972),
    el!("Fm", "Fermium", 257.0, 257.095099),
    el!("Md", "Mendelevium", 258.0, 258.098425),
    el!("No", "Nobelium", 259.0, 259.101024),
    el!("Lr", "Lawrencium", 266.0, 266.0),
    el!("Rf", "Rutherfordium", 267.0, 267.0),
    el!("Db", "Dubnium", 268.0, 268.0),
    el!("Sg", "Seaborgium", 269.0, 269.0),
    el!("Bh", "Bohrium", 270.0, 270.0),
    el!("Hs", "Hassium", 277.0, 277.0),
    el!("Mt", "Meitnerium", 278.0, 278.0),
    el!("Ds", "Darmstadtium", 281.0, 281.0),
    el!("Rg", "Roentgenium", 282.0, 282.0),
    el!("Cn", "Copernicium", 285.0, 285.0),
    el!("Nh", "Nihonium", 286.0, 286.0),
    el!("Fl", "Flerovium", 289.0, 289.0),
    el!("Mc", "Moscovium", 290.0, 290.0),
    el!("Lv", "Livermorium", 293.0, 293.0),
    el!("Ts", "Tennessine", 294.0, 294.0),
    el!("Og", "Oganesson", 294.0, 294.0),
];

pub const H: u8 = 1;
pub const C: u8 = 6;
pub const N: u8 = 7;
pub const O: u8 = 8;
pub const S: u8 = 16;

pub fn symbol(z: u8) -> &'static str {
    ELEMENTS.get(z as usize).map(|e| e.sym).unwrap_or("*")
}

pub fn by_symbol(s: &str) -> Option<u8> {
    ELEMENTS
        .iter()
        .position(|e| e.sym == s)
        .filter(|&i| i > 0)
        .map(|i| i as u8)
}

/// Case-forgiving lookup ("cl" -> Cl).
pub fn by_symbol_loose(s: &str) -> Option<u8> {
    let mut c = s.chars();
    let first = c.next()?;
    let fixed: String = first.to_uppercase().chain(c.flat_map(|x| x.to_lowercase())).collect();
    by_symbol(&fixed)
}

/// Normal valences used to work out implicit hydrogens (empty: none added).
pub fn valences(z: u8) -> &'static [i32] {
    match z {
        1 => &[1],
        5 => &[3],
        6 => &[4],
        7 => &[3, 5],
        8 => &[2],
        9 => &[1],
        14 => &[4],
        15 => &[3, 5],
        16 => &[2, 4, 6],
        17 | 35 | 53 => &[1, 3, 5, 7],
        32 => &[4],
        33 => &[3, 5],
        34 => &[2, 4, 6],
        52 => &[2, 4, 6],
        _ => &[],
    }
}

/// Valences after a formal charge is applied.
pub fn charged_valences(z: u8, charge: i32) -> Vec<i32> {
    let base = valences(z);
    if charge == 0 {
        return base.to_vec();
    }
    match z {
        // Hexacoordinate anions: PF6⁻, AsF6⁻, SbF6⁻ (and SiF6²⁻ below).
        15 | 33 | 51 if charge == -1 => vec![2, 4, 6],
        14 if charge == -2 => vec![6],
        6 | 14 | 32 => base.iter().map(|v| v - charge.abs()).collect(),
        5 => base.iter().map(|v| v - charge).collect(),
        7 | 8 | 9 | 15 | 16 | 17 | 33 | 34 | 35 | 52 | 53 => base.iter().map(|v| v + charge).filter(|v| *v >= 0).collect(),
        _ => vec![],
    }
}

/// Group/period position for the periodic table grid: (row, column), 0-based.
/// Rows 7 and 8 hold the lanthanides and actinides.
pub fn grid_pos(z: u8) -> (usize, usize) {
    let z = z as usize;
    match z {
        1 => (0, 0),
        2 => (0, 17),
        3..=4 => (1, z - 3),
        5..=10 => (1, z - 5 + 12),
        11..=12 => (2, z - 11),
        13..=18 => (2, z - 13 + 12),
        19..=36 => (3, z - 19),
        37..=54 => (4, z - 37),
        55..=56 => (5, z - 55),
        57..=71 => (7, z - 57 + 3),
        72..=86 => (5, z - 72 + 3),
        87..=88 => (6, z - 87),
        89..=103 => (8, z - 89 + 3),
        104..=118 => (6, z - 104 + 3),
        _ => (0, 0),
    }
}

/// Colour family for the periodic table and atom labels.
pub fn category(z: u8) -> u8 {
    match z {
        1 | 6 | 7 | 8 | 15 | 16 | 34 => 0,          // non-metal
        2 | 10 | 18 | 36 | 54 | 86 | 118 => 1,      // noble gas
        3 | 11 | 19 | 37 | 55 | 87 => 2,            // alkali
        4 | 12 | 20 | 38 | 56 | 88 => 3,            // alkaline earth
        5 | 14 | 32 | 33 | 51 | 52 => 4,            // metalloid
        9 | 17 | 35 | 53 | 85 | 117 => 5,           // halogen
        57..=71 => 6,
        89..=103 => 7,
        13 | 31 | 49 | 50 | 81 | 82 | 83 | 84 | 113 | 114 | 115 | 116 => 8, // post-transition
        _ => 9,                                      // transition metal
    }
}

/// Isotopes used as labels and tracers besides the natural ones listed in `isotopes`:
/// (element, mass number, exact mass).
const EXTRA_ISOTOPES: &[(u8, u16, f64)] = &[
    (1, 3, 3.016049),
    (6, 11, 11.011433),
    (6, 14, 14.003242),
    (7, 13, 13.005739),
    (8, 15, 15.003066),
    (9, 18, 18.000938),
    (9, 19, 18.998403),
    (11, 22, 21.994437),
    (11, 23, 22.989769),
    (15, 31, 30.973762),
    (15, 32, 31.973908),
    (15, 33, 32.971726),
    (16, 35, 34.969032),
    (20, 45, 44.956186),
    (24, 51, 50.944767),
    (26, 54, 53.939611),
    (26, 56, 55.934937),
    (26, 57, 56.935393),
    (26, 59, 58.934875),
    (27, 60, 59.933817),
    (29, 64, 63.929764),
    (31, 68, 67.927980),
    (38, 89, 88.907451),
    (39, 90, 89.907152),
    (40, 89, 88.908890),
    (43, 99, 98.906255),
    (49, 111, 110.905103),
    (53, 123, 122.905590),
    (53, 124, 123.906210),
    (53, 125, 124.904630),
    (53, 127, 126.904473),
    (53, 131, 130.906125),
    (55, 137, 136.907090),
    (78, 195, 194.964791),
    (81, 201, 200.970819),
    (50, 119, 118.903308),
    (82, 207, 206.975897),
    (80, 199, 198.968279),
];

/// Exact mass of mass number `a` of element `z`, if it is a known isotope.
pub fn isotope_mass(z: u8, a: u16) -> Option<f64> {
    if let Some(m) = isotopes(z).iter().map(|x| x.0).find(|m| m.round() as u16 == a) {
        return Some(m);
    }
    EXTRA_ISOTOPES.iter().find(|x| x.0 == z && x.1 == a).map(|x| x.2)
}

/// Isotopes offered for an element, lightest first (natural ones and common tracers).
pub fn known_isotopes(z: u8) -> Vec<u16> {
    let mut v: Vec<u16> = EXTRA_ISOTOPES.iter().filter(|x| x.0 == z).map(|x| x.1).collect();
    v.extend(isotopes(z).iter().map(|x| x.0.round() as u16));
    v.sort();
    v.dedup();
    v
}

/// Every isotope that can be a formula entry of its own, in a fixed order.
fn isotope_table() -> &'static Vec<(u8, u16, f64)> {
    static T: std::sync::OnceLock<Vec<(u8, u16, f64)>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut v = Vec::new();
        for z in 1..ELEMENTS.len().min(119) as u8 {
            // Elements with a single known isotope have nothing to label.
            let known = known_isotopes(z);
            if known.len() < 2 {
                continue;
            }
            for a in known {
                if let Some(m) = isotope_mass(z, a) {
                    v.push((z, a, m));
                }
            }
        }
        v.truncate(127);
        v
    })
}

/// Formula key of an isotope-labelled atom (128 and up), so that, for example, ¹³C is counted
/// apart from natural carbon. None for unknown isotopes.
pub fn iso_key(z: u8, a: u16) -> Option<u8> {
    isotope_table().iter().position(|x| x.0 == z && x.1 == a).map(|i| 128 + i as u8)
}

/// Element, mass number and exact mass of a formula key that names an isotope.
pub fn iso_of_key(k: u8) -> Option<(u8, u16, f64)> {
    if k < 128 {
        return None;
    }
    isotope_table().get((k - 128) as usize).copied()
}

/// Element of a formula key (an isotope key gives its element).
pub fn key_element(k: u8) -> u8 {
    iso_of_key(k).map(|x| x.0).unwrap_or(k)
}

/// Exact (monoisotopic) mass for a formula key.
pub fn key_mono(k: u8) -> f64 {
    match iso_of_key(k) {
        Some(x) => x.2,
        None => ELEMENTS.get(k as usize).map(|e| e.mono).unwrap_or(0.0),
    }
}

/// Average mass for a formula key (a labelled atom weighs its isotope's mass).
pub fn key_mass(k: u8) -> f64 {
    match iso_of_key(k) {
        Some(x) => x.2,
        None => ELEMENTS.get(k as usize).map(|e| e.mass).unwrap_or(0.0),
    }
}

/// Mass number written as superscript digits ("¹³").
pub fn superscript_number(a: u16) -> String {
    const SUP: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    a.to_string().chars().map(|c| SUP[c as usize - '0' as usize]).collect()
}

/// Symbol of a formula key: "C", "¹³C", "D" for ²H, "T" for ³H.
pub fn key_symbol(k: u8) -> String {
    match iso_of_key(k) {
        Some((1, 2, _)) => "D".into(),
        Some((1, 3, _)) => "T".into(),
        Some((z, a, _)) => format!("{}{}", superscript_number(a), symbol(z)),
        None => symbol(k).to_string(),
    }
}

/// Natural isotopes (mass, abundance %) for common elements, used for the m/z pattern.
pub fn isotopes(z: u8) -> Vec<(f64, f64)> {
    match z {
        1 => vec![(1.007825, 99.9885), (2.014102, 0.0115)],
        5 => vec![(10.012937, 19.9), (11.009305, 80.1)],
        6 => vec![(12.0, 98.93), (13.003355, 1.07)],
        7 => vec![(14.003074, 99.636), (15.000109, 0.364)],
        8 => vec![(15.994915, 99.757), (16.999132, 0.038), (17.999160, 0.205)],
        14 => vec![(27.976927, 92.223), (28.976495, 4.685), (29.973770, 3.092)],
        16 => vec![(31.972071, 94.99), (32.971459, 0.75), (33.967867, 4.25), (35.967081, 0.01)],
        17 => vec![(34.968853, 75.76), (36.965903, 24.24)],
        35 => vec![(78.918338, 50.69), (80.916291, 49.31)],
        3 => vec![(6.015122, 7.59), (7.016004, 92.41)],
        12 => vec![(23.985042, 78.99), (24.985837, 10.0), (25.982593, 11.01)],
        19 => vec![(38.963707, 93.2581), (40.961826, 6.7302)],
        34 => vec![(75.919214, 9.37), (76.919914, 7.63), (77.917309, 23.77), (79.916522, 49.61), (81.916700, 8.73)],
        _ => {
            let e = &ELEMENTS[z as usize];
            vec![(e.mono, 100.0)]
        }
    }
}

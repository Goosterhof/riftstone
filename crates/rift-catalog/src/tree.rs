//! The addon tree across the whole library: `addonfor` parents, breadcrumb
//! trails, menu-only detection ([`resolve`]) — and one package's options
//! nested the way their menus nest them ([`nest`]), the shape of the window's
//! library tree.
//!
//! **Parents.** An option's `addon_for` names its parent by display name
//! (case-insensitive, trimmed). The parent is looked up in the option's own
//! package first, then across the library in library order (the order of the
//! slice). When several options of its own package carry the name, the one
//! whose folder holds the option's own folder wins (the deepest such folder),
//! else the first: a collection with a "Hoods" menu for each wardrobe keeps
//! every variant under its own menu, and a nested variant (see
//! [`crate::classify`]) always lands under the modinfo option above it. A name
//! nobody carries leaves `parent: None`, keeps the raw `addon_for`, and files
//! one `MissingParent` problem on the package per distinct missing name,
//! counting the options that wait under it.
//!
//! **Cycles.** Parents are assigned in library order; an assignment that would
//! close a loop (`A → B → A`, or an option naming itself) is refused, so the
//! option that closes the cycle stays at the top level.
//!
//! **Trails.** `trail` = the package display name, then the option's own
//! bundle (`NameAsBundle`), then every ancestor from the top of the menu down
//! to the parent: `["Vernworth Cloaks Collection", "Cloaks", "Ermine Cape (Menu
//! Option)"]`. A crumb equal to the one before it (a package named like its own
//! top menu) is not repeated.
//!
//! **Idempotent.** Cached packages arrive already resolved, so `resolve`
//! first forgets its previous answers (`MissingParent` problems, parents,
//! trails) and then works them out again.
//!
//! **Nesting** ([`nest`], pure). An option whose parent is in its own package
//! hangs under that parent; an option whose parent is in another package, or
//! that has none, is a top node of its own package. At each level, two or more
//! siblings sharing a bundle (`NameAsBundle`, compared case-blind) gather under
//! one [`Nest::Bundle`], at the place of the first of them. Everything keeps
//! the package's option order, and every option appears exactly once: a loop
//! [`resolve`] would have broken (a package that was never resolved) is shown
//! once, from its first member.

use std::cmp::Reverse;
use std::collections::HashMap;

use rift_core::path::is_within;
use rift_core::{lower_key, ModOption, OptionId, Package, Problem, ProblemKind};

/// (package index, option index).
type Slot = (usize, usize);

/// Resolve `parent`, `trail` and `menu_only` for every option of every
/// package, in place.
pub fn resolve(packages: &mut [Package]) {
    for package in packages.iter_mut() {
        package
            .problems
            .retain(|problem| problem.kind != ProblemKind::MissingParent);
    }

    let mut by_name: HashMap<String, Vec<Slot>> = HashMap::new();
    for (pi, package) in packages.iter().enumerate() {
        for (oi, option) in package.options.iter().enumerate() {
            by_name
                .entry(name_key(&option.name))
                .or_default()
                .push((pi, oi));
        }
    }

    let mut parents: Vec<Vec<Option<Slot>>> = packages
        .iter()
        .map(|p| vec![None; p.options.len()])
        .collect();
    // Per package: (name key, the name as first written, how many options wait under it).
    let mut missing: Vec<Vec<(String, String, usize)>> = vec![Vec::new(); packages.len()];

    for (pi, package) in packages.iter().enumerate() {
        for (oi, option) in package.options.iter().enumerate() {
            let Some(raw) = option.addon_for.as_deref() else {
                continue;
            };
            let key = name_key(raw);
            if key.is_empty() {
                continue;
            }
            let me = (pi, oi);
            let slots = by_name.get(&key).map(Vec::as_slice).unwrap_or(&[]);
            let found = enclosing_namesake(packages, me, slots)
                .or_else(|| {
                    slots
                        .iter()
                        .copied()
                        .find(|&slot| slot.0 == pi && slot != me)
                })
                .or_else(|| slots.iter().copied().find(|&slot| slot != me));
            match found {
                Some(parent) if !reaches(&parents, parent, me) => parents[pi][oi] = Some(parent),
                Some(_) => log::warn!(
                    "{:?} in {:?} hangs under {raw:?}, which would close a menu loop; it stays at the top level",
                    option.name,
                    package.file_name
                ),
                None if slots.contains(&me) => {
                    log::warn!("{:?} in {:?} names itself as its menu parent", option.name, package.file_name)
                }
                None => match missing[pi].iter_mut().find(|(k, _, _)| *k == key) {
                    Some((_, _, waiting)) => *waiting += 1,
                    None => missing[pi].push((key, raw.trim().to_string(), 1)),
                },
            }
        }
    }

    let ids: Vec<Vec<OptionId>> = packages
        .iter()
        .map(|package| {
            package
                .options
                .iter()
                .map(|option| option.id.clone())
                .collect()
        })
        .collect();

    let mut trails: Vec<Vec<Vec<String>>> = packages
        .iter()
        .enumerate()
        .map(|(pi, package)| {
            package
                .options
                .iter()
                .enumerate()
                .map(|(oi, option)| {
                    let mut ancestors = Vec::new();
                    let mut current = parents[pi][oi];
                    while let Some((cp, co)) = current {
                        ancestors.push(packages[cp].options[co].name.as_str());
                        current = parents[cp][co];
                    }
                    let mut trail = Vec::with_capacity(ancestors.len() + 2);
                    push_crumb(&mut trail, &package.display_name);
                    if let Some(bundle) = option.bundle.as_deref() {
                        push_crumb(&mut trail, bundle);
                    }
                    for name in ancestors.into_iter().rev() {
                        push_crumb(&mut trail, name);
                    }
                    trail
                })
                .collect()
        })
        .collect();

    for (pi, package) in packages.iter_mut().enumerate() {
        for (oi, option) in package.options.iter_mut().enumerate() {
            option.parent = parents[pi][oi].map(|(cp, co)| ids[cp][co].clone());
            option.trail = std::mem::take(&mut trails[pi][oi]);
            option.menu_only =
                option.info.as_ref().is_some_and(|info| info.dummy) || option.payload.is_empty();
        }
        for (_, name, waiting) in &missing[pi] {
            package
                .problems
                .push(missing_parent_problem(name, *waiting));
        }
    }
}

/// Among the namesakes in `slots`, the one in `me`'s own package whose folder
/// holds `me`'s folder — the deepest such folder when several do.
fn enclosing_namesake(packages: &[Package], me: Slot, slots: &[Slot]) -> Option<Slot> {
    let options = &packages[me.0].options;
    let own_root = options[me.1].root.as_str();
    slots
        .iter()
        .copied()
        .filter(|&slot| slot.0 == me.0 && slot != me)
        .filter(|&slot| {
            let root = options[slot.1].root.as_str();
            lower_key(root) != lower_key(own_root) && is_within(own_root, root)
        })
        .min_by_key(|&slot| Reverse(options[slot.1].root.len()))
}

/// Does walking up from `from` reach `target`? (The assigned parents never
/// form a loop, so the walk ends.)
fn reaches(parents: &[Vec<Option<Slot>>], from: Slot, target: Slot) -> bool {
    let mut current = Some(from);
    while let Some(slot) = current {
        if slot == target {
            return true;
        }
        current = parents[slot.0][slot.1];
    }
    false
}

fn name_key(name: &str) -> String {
    lower_key(name.trim())
}

fn push_crumb(trail: &mut Vec<String>, crumb: &str) {
    let crumb = crumb.trim();
    if crumb.is_empty()
        || trail
            .last()
            .is_some_and(|last| name_key(last) == name_key(crumb))
    {
        return;
    }
    trail.push(crumb.to_string());
}

fn missing_parent_problem(name: &str, waiting: usize) -> Problem {
    let (options, verb, shown) = if waiting == 1 {
        ("1 option".to_string(), "hangs", "it shows")
    } else {
        (format!("{waiting} options"), "hang", "they show")
    };
    Problem {
        kind: ProblemKind::MissingParent,
        message: format!(
            "{options} in this package {verb} under \"{name}\" (addonfor), but no option with that name is in \
             the mods folder, so {shown} at the top level. Add the mod that provides \"{name}\" to the mods \
             folder to restore the menu."
        ),
    }
}

// ---------------------------------------------------------------------------
// One package, nested
// ---------------------------------------------------------------------------

/// One node of a package's option tree ([`nest`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nest {
    /// An option, with the options that hang under it.
    Option { id: OptionId, children: Vec<Nest> },
    /// Two or more siblings sharing a `NameAsBundle` value, gathered under the
    /// first one's spelling of it.
    Bundle { label: String, children: Vec<Nest> },
}

/// A package's options nested the way their menus nest them (see the module
/// docs): its top nodes, each with its subtree. Pure; it reads `parent` and
/// `bundle` as they stand, so a resolved package nests as its menus do.
pub fn nest(package: &Package) -> Vec<Nest> {
    let options = &package.options;
    let mut slot_of: HashMap<&OptionId, usize> = HashMap::with_capacity(options.len());
    for (slot, option) in options.iter().enumerate() {
        slot_of.entry(&option.id).or_insert(slot);
    }
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); options.len()];
    let mut top = Vec::new();
    for (slot, option) in options.iter().enumerate() {
        match option
            .parent
            .as_ref()
            .and_then(|parent| slot_of.get(parent))
        {
            Some(&parent) => children[parent].push(slot),
            None => top.push(slot),
        }
    }

    let mut placed = vec![false; options.len()];
    let mut nodes = nest_level(options, &top, &children, &mut placed);
    // Options no top node reaches sit in a loop: each loop is shown once,
    // from its first member.
    for slot in 0..options.len() {
        if !placed[slot] {
            nodes.extend(nest_level(options, &[slot], &children, &mut placed));
        }
    }
    nodes
}

/// One level of siblings, bundles gathered at their first member's place.
fn nest_level(
    options: &[ModOption],
    level: &[usize],
    children: &[Vec<usize>],
    placed: &mut [bool],
) -> Vec<Nest> {
    // Each entry: the members of one place (an option alone, or a bundle).
    let mut places: Vec<Vec<usize>> = Vec::new();
    let mut bundle_place: HashMap<String, usize> = HashMap::new();
    for &slot in level.iter().filter(|&&slot| !placed[slot]) {
        match bundle_key(&options[slot]) {
            Some(key) => match bundle_place.get(&key) {
                Some(&at) => places[at].push(slot),
                None => {
                    bundle_place.insert(key, places.len());
                    places.push(vec![slot]);
                }
            },
            None => places.push(vec![slot]),
        }
    }

    let mut nodes = Vec::new();
    for members in places {
        let mut built: Vec<Nest> = members
            .iter()
            .filter_map(|&slot| nest_option(options, slot, children, placed))
            .collect();
        if built.len() >= 2 {
            let label = options[members[0]]
                .bundle
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .to_string();
            nodes.push(Nest::Bundle {
                label,
                children: built,
            });
        } else {
            nodes.append(&mut built);
        }
    }
    nodes
}

/// An option and its subtree; `None` when a loop already placed it.
fn nest_option(
    options: &[ModOption],
    slot: usize,
    children: &[Vec<usize>],
    placed: &mut [bool],
) -> Option<Nest> {
    if std::mem::replace(&mut placed[slot], true) {
        return None;
    }
    Some(Nest::Option {
        id: options[slot].id.clone(),
        children: nest_level(options, &children[slot], children, placed),
    })
}

/// The bundle an option gathers under (case-blind); none when blank.
fn bundle_key(option: &ModOption) -> Option<String> {
    option
        .bundle
        .as_deref()
        .map(|bundle| lower_key(bundle.trim()))
        .filter(|key| !key.is_empty())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rift_core::{
        DeployTarget, ModInfo, ModOption, PackageFormat, PackageId, PayloadFile, PayloadKinds,
    };

    use super::*;

    struct Spec<'a> {
        name: &'a str,
        addon_for: Option<&'a str>,
        bundle: Option<&'a str>,
        dummy: bool,
        payload: bool,
    }

    fn opt(name: &str) -> Spec<'_> {
        Spec {
            name,
            addon_for: None,
            bundle: None,
            dummy: false,
            payload: true,
        }
    }

    impl<'a> Spec<'a> {
        fn under(mut self, parent: &'a str) -> Self {
            self.addon_for = Some(parent);
            self
        }
        fn bundled(mut self, bundle: &'a str) -> Self {
            self.bundle = Some(bundle);
            self
        }
        fn menu(mut self) -> Self {
            self.dummy = true;
            self.payload = false;
            self
        }
        fn empty(mut self) -> Self {
            self.payload = false;
            self
        }
    }

    fn package(display_name: &str, specs: &[Spec<'_>]) -> Package {
        let id = PackageId::for_archive(&format!("{display_name}.zip"), 1);
        let options = specs
            .iter()
            .map(|spec| {
                let root = format!("{display_name}/{}", spec.name);
                let payload: Vec<PayloadFile> = if spec.payload {
                    vec![PayloadFile {
                        entry: format!("{root}/natives/stm/a.mesh.260421070"),
                        target: DeployTarget::Loose {
                            game_path: "natives/stm/a.mesh.260421070".into(),
                        },
                        size: 1,
                    }]
                } else {
                    Vec::new()
                };
                ModOption {
                    id: OptionId::new(&id, &root),
                    package_id: id.clone(),
                    root,
                    name: spec.name.to_string(),
                    info: Some(ModInfo {
                        name: Some(spec.name.to_string()),
                        addon_for: spec.addon_for.map(str::to_string),
                        name_as_bundle: spec.bundle.map(str::to_string),
                        dummy: spec.dummy,
                        ..ModInfo::default()
                    }),
                    screenshot: None,
                    kinds: PayloadKinds::tally(&payload),
                    menu_only: spec.dummy || payload.is_empty(),
                    payload,
                    skipped: Vec::new(),
                    parent: None,
                    addon_for: spec.addon_for.map(str::to_string),
                    bundle: spec.bundle.map(str::to_string),
                    trail: Vec::new(),
                }
            })
            .collect();
        Package {
            id,
            path: PathBuf::from(format!("/mods/{display_name}.zip")),
            file_name: format!("{display_name}.zip"),
            display_name: display_name.to_string(),
            format: PackageFormat::Zip,
            size: 1,
            modified_ms: 0,
            entry_count: 0,
            options,
            nexus: None,
            problems: Vec::new(),
        }
    }

    fn find<'a>(packages: &'a [Package], name: &str) -> &'a ModOption {
        packages
            .iter()
            .flat_map(|p| &p.options)
            .find(|o| o.name == name)
            .unwrap_or_else(|| panic!("no option {name:?}"))
    }

    fn parent_name<'a>(packages: &'a [Package], name: &str) -> Option<&'a str> {
        let parent = find(packages, name).parent.as_ref()?;
        packages
            .iter()
            .flat_map(|p| &p.options)
            .find(|o| &o.id == parent)
            .map(|o| o.name.as_str())
    }

    fn collection() -> Package {
        package(
            "Vernworth Cloaks Collection",
            &[
                opt("Cloaks").menu(),
                opt("Ermine Cape (Menu Option)").under("Cloaks").empty(),
                opt("Ermine Cape - White")
                    .under("ermine cape (menu option) ")
                    .bundled("Ermine Capes"),
                opt("Ermine Cape - Black").under("Ermine Cape (Menu Option)"),
            ],
        )
    }

    #[test]
    fn parents_resolve_within_a_package_and_trails_follow_the_menus() {
        let mut packages = vec![collection()];
        resolve(&mut packages);

        assert_eq!(parent_name(&packages, "Cloaks"), None);
        assert_eq!(
            parent_name(&packages, "Ermine Cape (Menu Option)"),
            Some("Cloaks")
        );
        assert_eq!(
            parent_name(&packages, "Ermine Cape - White"),
            Some("Ermine Cape (Menu Option)"),
            "case-insensitive and trimmed"
        );

        assert_eq!(
            find(&packages, "Ermine Cape - Black").trail,
            [
                "Vernworth Cloaks Collection",
                "Cloaks",
                "Ermine Cape (Menu Option)"
            ]
        );
        assert_eq!(
            find(&packages, "Ermine Cape - White").trail,
            [
                "Vernworth Cloaks Collection",
                "Ermine Capes",
                "Cloaks",
                "Ermine Cape (Menu Option)"
            ],
            "the bundle follows the package"
        );
        assert_eq!(
            find(&packages, "Cloaks").trail,
            ["Vernworth Cloaks Collection"]
        );
        assert!(find(&packages, "Ermine Cape (Menu Option)").menu_only);
        assert!(!find(&packages, "Ermine Cape - Black").menu_only);
        assert!(packages[0].problems.is_empty());
    }

    #[test]
    fn parents_resolve_across_packages_own_package_first_then_library_order() {
        let mut packages = vec![
            package(
                "Lantern Menus",
                &[opt("Lanterns").menu(), opt("Tops").menu()],
            ),
            package("Other Menus", &[opt("Tops").menu()]),
            package(
                "Lantern Glow Recolor",
                &[
                    opt("Ember Glow").under("LANTERNS"),
                    opt("Tops").menu(),
                    opt("Ember Top").under("Tops"),
                ],
            ),
            package("Vernworth Tops", &[opt("Ashen Top").under("Tops")]),
        ];
        resolve(&mut packages);

        let ember_glow = find(&packages, "Ember Glow");
        assert_eq!(
            ember_glow.parent.as_ref(),
            Some(&packages[0].options[0].id),
            "cross-package"
        );
        assert_eq!(ember_glow.trail, ["Lantern Glow Recolor", "Lanterns"]);

        assert_eq!(
            find(&packages, "Ember Top").parent.as_ref(),
            Some(&packages[2].options[1].id),
            "its own package's Tops wins over the library's first"
        );
        assert_eq!(
            find(&packages, "Ashen Top").parent.as_ref(),
            Some(&packages[0].options[1].id),
            "otherwise the first Tops in library order"
        );
    }

    #[test]
    fn a_missing_parent_is_reported_once_per_name() {
        let mut packages = vec![package(
            "Vernworth Tops",
            &[
                opt("Ashen Top").under("Ghost Menu"),
                opt("Amber Top").under("ghost menu"),
                opt("Azure Top").under(" Ghost Menu "),
                opt("Violet Top").under("Other Ghost"),
            ],
        )];
        resolve(&mut packages);

        let ashen = find(&packages, "Ashen Top");
        assert_eq!(ashen.parent, None);
        assert_eq!(
            ashen.addon_for.as_deref(),
            Some("Ghost Menu"),
            "the raw name is kept"
        );
        assert_eq!(ashen.trail, ["Vernworth Tops"]);

        let problems = &packages[0].problems;
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems
            .iter()
            .all(|p| p.kind == ProblemKind::MissingParent));
        assert!(
            problems[0]
                .message
                .starts_with("3 options in this package hang under \"Ghost Menu\""),
            "{}",
            problems[0].message
        );
        assert!(
            problems[1]
                .message
                .starts_with("1 option in this package hangs under \"Other Ghost\""),
            "{}",
            problems[1].message
        );
    }

    #[test]
    fn cycles_break_at_the_option_that_closes_them() {
        let mut packages = vec![package(
            "Loops",
            &[
                opt("Alpha").under("Beta"),
                opt("Beta").under("Alpha"),
                opt("Gamma").under("Delta"),
                opt("Delta").under("Epsilon"),
                opt("Epsilon").under("Gamma"),
                opt("Narcissus").under("narcissus"),
            ],
        )];
        resolve(&mut packages);

        assert_eq!(parent_name(&packages, "Alpha"), Some("Beta"));
        assert_eq!(parent_name(&packages, "Beta"), None, "Beta closes the loop");
        assert_eq!(parent_name(&packages, "Gamma"), Some("Delta"));
        assert_eq!(parent_name(&packages, "Delta"), Some("Epsilon"));
        assert_eq!(
            parent_name(&packages, "Epsilon"),
            None,
            "Epsilon closes the loop"
        );
        assert_eq!(parent_name(&packages, "Narcissus"), None);
        assert_eq!(
            find(&packages, "Gamma").trail,
            ["Loops", "Epsilon", "Delta"]
        );
        assert!(
            packages[0].problems.is_empty(),
            "a loop is not a missing parent"
        );
    }

    #[test]
    fn repeated_crumbs_collapse() {
        let mut packages = vec![package(
            "Lanterns",
            &[
                opt("Lanterns").menu(),
                opt("Ember Glow").under("Lanterns").bundled("lanterns"),
            ],
        )];
        resolve(&mut packages);
        assert_eq!(find(&packages, "Ember Glow").trail, ["Lanterns"]);
    }

    #[test]
    fn resolving_twice_changes_nothing() {
        let mut packages = vec![
            collection(),
            package("Vernworth Tops", &[opt("Ashen Top").under("Ghost Menu")]),
        ];
        resolve(&mut packages);
        let once = packages.clone();
        resolve(&mut packages);
        assert_eq!(
            packages, once,
            "no duplicated problems, same parents and trails"
        );
    }

    #[test]
    fn an_empty_option_without_children_stays_menu_only() {
        let mut packages = vec![package("Lonely", &[opt("Nothing Here").empty()])];
        resolve(&mut packages);
        assert!(find(&packages, "Nothing Here").menu_only);
    }

    /// Move option `index` of `package` to the folder `root`.
    fn move_to(package: &mut Package, index: usize, root: &str) {
        let option = &mut package.options[index];
        option.root = root.to_string();
        option.id = OptionId::new(&package.id, root);
    }

    #[test]
    fn among_namesakes_the_folder_that_holds_the_option_wins() {
        let mut wardrobe = package(
            "Vernworth Wardrobe",
            &[
                opt("Hoods").menu(),
                opt("Hoods").menu(),
                opt("Hoods").menu(),
                opt("Ashen Hood").under("Hoods"),
                opt("Amber Hood").under("hoods"),
            ],
        );
        move_to(&mut wardrobe, 0, "Vernworth Wardrobe/Female/Hoods");
        move_to(&mut wardrobe, 1, "Vernworth Wardrobe/Male");
        move_to(&mut wardrobe, 2, "Vernworth Wardrobe/MALE/Hoods");
        move_to(&mut wardrobe, 3, "Vernworth Wardrobe/Male/Hoods/Ashen Hood");
        move_to(&mut wardrobe, 4, "Vernworth Wardrobe/Loose/Amber Hood");
        let ids: Vec<OptionId> = wardrobe.options.iter().map(|o| o.id.clone()).collect();
        let mut packages = vec![wardrobe];
        resolve(&mut packages);

        assert_eq!(
            find(&packages, "Ashen Hood").parent.as_ref(),
            Some(&ids[2]),
            "the deepest enclosing namesake, case-blind"
        );
        assert_eq!(
            find(&packages, "Amber Hood").parent.as_ref(),
            Some(&ids[0]),
            "held by none of them: the first in the package, as before"
        );
    }

    // -----------------------------------------------------------------------
    // nest
    // -----------------------------------------------------------------------

    /// A package whose options already carry their parents: `(root, name,
    /// parent root, bundle)`, every parent in the same package.
    fn shelf(rows: &[(&str, &str, Option<&str>, Option<&str>)]) -> Package {
        let mut shelf = package("Vernworth Cloaks Collection", &[]);
        let id = shelf.id.clone();
        shelf.options = rows
            .iter()
            .map(|&(root, name, parent, bundle)| ModOption {
                id: OptionId::new(&id, root),
                package_id: id.clone(),
                root: root.to_string(),
                name: name.to_string(),
                info: None,
                screenshot: None,
                payload: Vec::new(),
                skipped: Vec::new(),
                kinds: PayloadKinds::default(),
                parent: parent.map(|parent| OptionId::new(&id, parent)),
                addon_for: None,
                bundle: bundle.map(str::to_string),
                menu_only: false,
                trail: Vec::new(),
            })
            .collect();
        shelf
    }

    /// The tree in one line: `name[children]` for an option, `{label}(members)`
    /// for a bundle.
    fn shape(package: &Package, nodes: &[Nest]) -> String {
        nodes
            .iter()
            .map(|node| match node {
                Nest::Option { id, children } => {
                    let name = &package
                        .options
                        .iter()
                        .find(|option| &option.id == id)
                        .expect("an option of the package")
                        .name;
                    if children.is_empty() {
                        name.clone()
                    } else {
                        format!("{name}[{}]", shape(package, children))
                    }
                }
                Nest::Bundle { label, children } => {
                    format!("{{{label}}}({})", shape(package, children))
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Moved here from the spine with the nesting itself (v0.3): the spine
    /// decorates these nodes into the window's tree.
    #[test]
    fn the_tree_nests_addons_and_gathers_bundles() {
        let package = shelf(&[
            ("Menu", "Cloaks", None, None),
            ("Menu/A", "Ermine Cape", Some("Menu"), Some("Capes")),
            (
                "Menu/B",
                "Ermine Cape - Variation 2",
                Some("Menu"),
                Some("capes"),
            ),
            ("Menu/C", "Wool Hood", Some("Menu"), None),
        ]);
        let option = |root: &str| Nest::Option {
            id: OptionId::new(&package.id, root),
            children: Vec::new(),
        };
        let tree = nest(&package);
        assert_eq!(tree.len(), 1);
        let Nest::Option { id, children } = &tree[0] else {
            panic!("an option at the top: {tree:?}");
        };
        assert_eq!(id, &OptionId::new(&package.id, "Menu"));
        assert_eq!(children.len(), 2, "one bundle node and one plain option");
        assert_eq!(
            children[0],
            Nest::Bundle {
                label: "Capes".to_string(),
                children: vec![option("Menu/A"), option("Menu/B")],
            },
            "case-blind, under the first member's spelling"
        );
        assert_eq!(children[1], option("Menu/C"));
        assert_eq!(
            shape(&package, &tree),
            "Cloaks[{Capes}(Ermine Cape, Ermine Cape - Variation 2), Wool Hood]"
        );
    }

    #[test]
    fn a_parent_in_another_package_or_nowhere_makes_a_top_node() {
        let mut packages = vec![
            collection(),
            package(
                "Vernworth Trims",
                &[
                    opt("Ermine Trim").under("Cloaks"),
                    opt("Ghost Trim").under("Ghost Menu"),
                    opt("Fur Trim"),
                ],
            ),
        ];
        resolve(&mut packages);
        assert_eq!(
            shape(&packages[0], &nest(&packages[0])),
            "Cloaks[Ermine Cape (Menu Option)[Ermine Cape - White, Ermine Cape - Black]]",
            "a bundle of one is a plain option"
        );
        assert_eq!(
            shape(&packages[1], &nest(&packages[1])),
            "Ermine Trim, Ghost Trim, Fur Trim",
            "the trim's menu lives in the collection; the ghost's nowhere"
        );
        assert!(nest(&package("Empty", &[])).is_empty());
    }

    #[test]
    fn bundles_gather_at_their_first_place_and_only_among_siblings() {
        let package = shelf(&[
            ("A", "Ashen Hood", None, Some("Hoods")),
            ("B", "Lantern", None, None),
            ("C", "Amber Hood", None, Some(" HOODS ")),
            ("D", "Hood Menu", None, None),
            ("D/E", "Azure Hood", Some("D"), Some("Hoods")),
            ("D/F", "Violet Hood", Some("D"), None),
            ("G", "Grey Hood", None, Some("  ")),
        ]);
        assert_eq!(
            shape(&package, &nest(&package)),
            "{Hoods}(Ashen Hood, Amber Hood), Lantern, Hood Menu[Azure Hood, Violet Hood], Grey Hood",
            "the package's order; the hood under the menu has no sibling in its bundle; a blank \
             bundle is none"
        );
    }

    #[test]
    fn a_loop_shows_each_option_once_and_loses_none() {
        let package = shelf(&[
            ("A", "Alpha", Some("B"), None),
            ("B", "Beta", Some("A"), None),
            ("C", "Gamma", None, None),
            ("D", "Narcissus", Some("D"), None),
            ("E", "Echo", Some("B"), None),
        ]);
        assert_eq!(
            shape(&package, &nest(&package)),
            "Gamma, Alpha[Beta[Echo]], Narcissus",
            "the top nodes first, then each loop from its first member"
        );
    }
}

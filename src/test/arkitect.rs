#[cfg(test)]
mod tests {
    use rust_arkitect::dsl::architectural_rules::{
        ArchitecturalRules, SubjectInjectableRuleBuilder,
    };
    use rust_arkitect::dsl::arkitect::Arkitect;
    use rust_arkitect::dsl::project::Project;
    use rust_arkitect::rule::Rule;
    use rust_arkitect::rust_file::RustFile;
    use std::collections::{HashMap, HashSet};
    use std::fmt;
    use syn::{
        Attribute, ExprPath, Item, ItemMod, Path, TypePath, UseTree,
        visit::{self, Visit},
    };

    #[test]
    // Custom rule types and AST helpers are ported ahead of the phases that
    // use them (4–5); until then they are intentionally unreferenced.
    #[allow(dead_code)]
    fn test_architectural_rules() {
        let project = Project::from_current_crate();
        let domain_deps = vec!["serde"];
        let infra_deps = vec![
            "prometheus",
            "reqwest",
            "sentry",
            "serde",
            "std",
            "vardy::domain",
        ];

        let interfaces_deps = vec![
            "axum",
            "crate::app",
            "crate::test",
            "minijinja",
            "serde_json",
            "std",
            "tower_http",
            "vardy::app",
            "vardy::domain",
            "vardy::infra",
            "vardy::test",
            // just for tests
            "sqlx",
        ];

        let rules = ArchitecturalRules::define()
            .rules_for_module("vardy::domain")
            .it_must_not_depend_on(&["vardy::app", "vardy::infra", "vardy::interfaces"])
            .and_it_may_depend_on(&domain_deps)
            .rules_for_module("vardy::app")
            .it_must_not_depend_on(&["vardy::interfaces"])
            .rules_for_module("vardy::infra")
            .it_must_not_depend_on(&["vardy::app", "vardy::interfaces"])
            .and_it_may_depend_on(&infra_deps)
            .rules_for_module("vardy::interfaces")
            .it_may_depend_on(&interfaces_deps)
            .and_it(Box::new(MustNotDependOnExceptTestsBuilder {
                forbidden: vec!["sqlx".to_string(), "reqwest".to_string()],
            }))
            .build();

        let result = Arkitect::ensure_that(project).complies_with(rules);

        assert!(
            result.is_ok(),
            "Detected violations:\n{}",
            result.err().unwrap().join("\n")
        );
        #[cfg(test)]
        pub struct MustNotDependOnExceptTests {
            subject: String,
            forbidden: Vec<String>,
        }

        #[cfg(test)]
        pub struct MustNotDependOnExceptTestsBuilder {
            pub forbidden: Vec<String>,
        }

        #[cfg(test)]
        impl SubjectInjectableRuleBuilder for MustNotDependOnExceptTestsBuilder {
            fn for_subject(&self, subject: &str) -> Box<dyn Rule> {
                Box::new(MustNotDependOnExceptTests {
                    subject: subject.to_string(),
                    forbidden: self.forbidden.clone(),
                })
            }
        }

        #[cfg(test)]
        impl fmt::Display for MustNotDependOnExceptTests {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    f,
                    "{} must not depend on {:?} (except in #[cfg(test)] modules)",
                    self.subject, self.forbidden
                )
            }
        }

        #[cfg(test)]
        impl Rule for MustNotDependOnExceptTests {
            fn is_applicable(&self, file: &RustFile) -> bool {
                file.logical_path.starts_with(&self.subject)
            }

            fn apply(&self, file: &RustFile) -> Result<(), String> {
                let deps = deps_outside_test_modules(&file.ast, &file.logical_path);
                let violations: Vec<_> = deps
                    .iter()
                    .filter(|d| {
                        self.forbidden
                            .iter()
                            .any(|f| *d == f.as_str() || d.starts_with(&format!("{}::", f)))
                    })
                    .collect();
                if violations.is_empty() {
                    Ok(())
                } else {
                    Err(format!(
                        "Forbidden dependencies in {}: {:?}",
                        file.path, violations
                    ))
                }
            }
        }

        // ---------------------------------------------------------------------------
        // AST helpers
        // ---------------------------------------------------------------------------

        #[cfg(test)]
        fn has_cfg_test(attrs: &[Attribute]) -> bool {
            attrs.iter().any(|attr| {
                attr.path().is_ident("cfg")
                    && attr
                        .meta
                        .require_list()
                        .map(|list| list.tokens.to_string().trim() == "test")
                        .unwrap_or(false)
            })
        }

        #[cfg(test)]
        /// Collect dependencies from the parts of the AST that are **not** inside
        /// `#[cfg(test)]` modules or behind `#[cfg(test)]` gates.
        fn deps_outside_test_modules(ast: &syn::File, logical_path: &str) -> Vec<String> {
            let crate_name = logical_path.split("::").next().unwrap_or("");
            let mut deps = Vec::new();
            let mut aliases: HashMap<String, String> = HashMap::new();

            for item in &ast.items {
                if has_cfg_test(item_attrs(item)) {
                    continue;
                }
                match item {
                    Item::Use(use_item) => {
                        collect_use_tree(
                            &use_item.tree,
                            &mut deps,
                            &mut aliases,
                            logical_path,
                            crate_name,
                            "",
                        );
                    }
                    Item::Mod(mod_item) if !has_cfg_test(&mod_item.attrs) => {
                        collect_mod_items(
                            mod_item,
                            &mut deps,
                            &mut aliases,
                            logical_path,
                            crate_name,
                        );
                    }
                    _ => {}
                }
            }

            // Path references in code (function bodies, type annotations, etc.)
            let mut visitor = PathCollector {
                deps: Vec::new(),
                aliases: &aliases,
                logical_path,
                crate_name,
                inside_test: false,
            };
            visitor.visit_file(ast);
            deps.extend(visitor.deps);

            let mut seen = HashSet::new();
            deps.into_iter()
                .filter(|d| seen.insert(d.clone()))
                .collect()
        }

        #[cfg(test)]
        fn item_attrs(item: &Item) -> &[Attribute] {
            match item {
                Item::Const(i) => &i.attrs,
                Item::Enum(i) => &i.attrs,
                Item::ExternCrate(i) => &i.attrs,
                Item::Fn(i) => &i.attrs,
                Item::ForeignMod(i) => &i.attrs,
                Item::Impl(i) => &i.attrs,
                Item::Macro(i) => &i.attrs,
                Item::Mod(i) => &i.attrs,
                Item::Static(i) => &i.attrs,
                Item::Struct(i) => &i.attrs,
                Item::Trait(i) => &i.attrs,
                Item::TraitAlias(i) => &i.attrs,
                Item::Type(i) => &i.attrs,
                Item::Union(i) => &i.attrs,
                Item::Use(i) => &i.attrs,
                Item::Verbatim(_) => &[],
                _ => &[],
            }
        }

        #[cfg(test)]
        fn collect_use_tree(
            tree: &UseTree,
            deps: &mut Vec<String>,
            aliases: &mut HashMap<String, String>,
            logical_path: &str,
            crate_name: &str,
            prefix: &str,
        ) {
            match tree {
                UseTree::Path(use_path) => {
                    let ident = use_path.ident.to_string();
                    if ident == "super" {
                        let parent = logical_path.rsplit_once("::").map(|x| x.0).unwrap_or("");
                        collect_use_tree(
                            &use_path.tree,
                            deps,
                            aliases,
                            logical_path,
                            crate_name,
                            parent,
                        );
                    } else if ident == "crate" {
                        collect_use_tree(
                            &use_path.tree,
                            deps,
                            aliases,
                            logical_path,
                            crate_name,
                            crate_name,
                        );
                    } else {
                        let new_prefix = if prefix.is_empty() {
                            ident
                        } else {
                            format!("{}::{}", prefix, ident)
                        };
                        collect_use_tree(
                            &use_path.tree,
                            deps,
                            aliases,
                            logical_path,
                            crate_name,
                            &new_prefix,
                        );
                    }
                }
                UseTree::Group(group) => {
                    for item in &group.items {
                        collect_use_tree(item, deps, aliases, logical_path, crate_name, prefix);
                    }
                }
                UseTree::Name(name) => {
                    let dep = format!("{}::{}", prefix, name.ident);
                    deps.push(dep.clone());
                    aliases.insert(name.ident.to_string(), dep);
                }
                UseTree::Glob(_) => {
                    deps.push(format!("{}::*", prefix));
                }
                UseTree::Rename(rename) => {
                    let dep = format!("{}::{}", prefix, rename.ident);
                    deps.push(dep.clone());
                    aliases.insert(rename.rename.to_string(), dep);
                }
            }
        }

        #[cfg(test)]
        fn collect_mod_items(
            mod_item: &ItemMod,
            deps: &mut Vec<String>,
            aliases: &mut HashMap<String, String>,
            logical_path: &str,
            crate_name: &str,
        ) {
            if let Some((_, items)) = &mod_item.content {
                let mod_path = format!("{}::{}", logical_path, mod_item.ident);
                for item in items {
                    if has_cfg_test(item_attrs(item)) {
                        continue;
                    }
                    match item {
                        Item::Use(use_item) => {
                            collect_use_tree(
                                &use_item.tree,
                                deps,
                                aliases,
                                &mod_path,
                                crate_name,
                                "",
                            );
                        }
                        Item::Mod(nested) if !has_cfg_test(&nested.attrs) => {
                            collect_mod_items(nested, deps, aliases, &mod_path, crate_name);
                        }
                        _ => {}
                    }
                }
            }
        }

        // Visitor that collects path references, skipping `#[cfg(test)]` modules.
        #[cfg(test)]
        struct PathCollector<'a> {
            deps: Vec<String>,
            aliases: &'a HashMap<String, String>,
            logical_path: &'a str,
            crate_name: &'a str,
            inside_test: bool,
        }

        #[cfg(test)]
        impl<'ast, 'a> Visit<'ast> for PathCollector<'a> {
            fn visit_item_mod(&mut self, node: &'ast ItemMod) {
                if has_cfg_test(&node.attrs) {
                    let prev = self.inside_test;
                    self.inside_test = true;
                    visit::visit_item_mod(self, node);
                    self.inside_test = prev;
                } else {
                    visit::visit_item_mod(self, node);
                }
            }

            fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
                if has_cfg_test(&node.attrs) {
                    let prev = self.inside_test;
                    self.inside_test = true;
                    visit::visit_item_fn(self, node);
                    self.inside_test = prev;
                } else {
                    visit::visit_item_fn(self, node);
                }
            }

            fn visit_expr_path(&mut self, node: &'ast ExprPath) {
                if !self.inside_test
                    && let Some(dep) =
                        resolve_path(&node.path, self.aliases, self.logical_path, self.crate_name)
                {
                    self.deps.push(dep);
                }
                visit::visit_expr_path(self, node);
            }

            fn visit_type_path(&mut self, node: &'ast TypePath) {
                if !self.inside_test
                    && node.path.segments.len() > 1
                    && let Some(dep) =
                        resolve_path(&node.path, self.aliases, self.logical_path, self.crate_name)
                {
                    self.deps.push(dep);
                }
                visit::visit_type_path(self, node);
            }
        }

        #[cfg(test)]
        fn resolve_path(
            path: &Path,
            aliases: &HashMap<String, String>,
            logical_path: &str,
            crate_name: &str,
        ) -> Option<String> {
            let first = path.segments.first()?.ident.to_string();
            let rest: Vec<String> = path
                .segments
                .iter()
                .skip(1)
                .map(|s| s.ident.to_string())
                .collect();
            match first.as_str() {
                "crate" => Some(if rest.is_empty() {
                    crate_name.to_string()
                } else {
                    format!("{}::{}", crate_name, rest.join("::"))
                }),
                "super" => {
                    let parent = logical_path.rsplit_once("::").map(|x| x.0).unwrap_or("");
                    Some(if rest.is_empty() {
                        parent.to_string()
                    } else {
                        format!("{}::{}", parent, rest.join("::"))
                    })
                }
                "self" => Some(logical_path.to_string()),
                other => {
                    if let Some(alias_target) = aliases.get(other) {
                        Some(if rest.is_empty() {
                            alias_target.clone()
                        } else {
                            format!("{}::{}", alias_target, rest.join("::"))
                        })
                    } else if path.segments.len() > 1 {
                        Some(
                            path.segments
                                .iter()
                                .map(|s| s.ident.to_string())
                                .collect::<Vec<_>>()
                                .join("::"),
                        )
                    } else {
                        None
                    }
                }
            }
        }
    }
}

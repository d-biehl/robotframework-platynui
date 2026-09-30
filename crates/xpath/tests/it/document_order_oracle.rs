//! The engine against a naive evaluator on every ordered tree of up to six elements named `a` or
//! `b`, on the keyed `SimpleNode` and on its keyless view. The naive evaluator shares no code with
//! the engine: it lists each axis from each context by pre-order index, applies the predicates per
//! context, then sorts and deduplicates, as `XPath` 2.0 defines a path.

use std::rc::Rc;

use platynui_xpath::compiler::ir::CompiledXPath;
use platynui_xpath::engine::runtime::FunctionImplementations;
use platynui_xpath::functions::default_function_registry;
use platynui_xpath::xdm::XdmItem;
use platynui_xpath::{DynamicContextBuilder, SimpleNode, XdmNode, compile, evaluate};

use crate::common::{Keyless, labels, tree};

#[derive(Clone, Copy)]
enum Axis {
    Child,
    Descendant,
    DescendantOrSelf,
    Parent,
    Ancestor,
    FollowingSibling,
    Following,
    Preceding,
}

#[derive(Clone, Copy)]
enum Test {
    Name(char),
    AnyElement,
    AnyNode,
}

#[derive(Clone, Copy)]
enum Predicate {
    Position(usize),
    Last,
    HasChildB,
}

enum Step {
    Axis(Axis, Test, &'static [Predicate]),
    /// A parenthesized union of steps with the predicates on it, as in `(a|b)[1]`.
    Union(&'static [(Axis, Test)], &'static [Predicate]),
}

struct Case {
    xpath: &'static str,
    steps: &'static [Step],
    /// Predicates on the parenthesized whole path, as in `(//a)[2]`.
    outer: &'static [Predicate],
}

const DOUBLE_SLASH: Step = Step::Axis(Axis::DescendantOrSelf, Test::AnyNode, &[]);
const CHILD_A: Step = Step::Axis(Axis::Child, Test::Name('a'), &[]);

const CASES: &[Case] = &[
    Case { xpath: "//a", steps: &[DOUBLE_SLASH, CHILD_A], outer: &[] },
    Case {
        xpath: "//a[1]",
        steps: &[DOUBLE_SLASH, Step::Axis(Axis::Child, Test::Name('a'), &[Predicate::Position(1)])],
        outer: &[],
    },
    Case {
        xpath: "//a[last()]",
        steps: &[DOUBLE_SLASH, Step::Axis(Axis::Child, Test::Name('a'), &[Predicate::Last])],
        outer: &[],
    },
    Case { xpath: "(//a)[2]", steps: &[DOUBLE_SLASH, CHILD_A], outer: &[Predicate::Position(2)] },
    Case { xpath: "//a/b", steps: &[DOUBLE_SLASH, CHILD_A, Step::Axis(Axis::Child, Test::Name('b'), &[])], outer: &[] },
    Case {
        xpath: "//a/b[1]",
        steps: &[DOUBLE_SLASH, CHILD_A, Step::Axis(Axis::Child, Test::Name('b'), &[Predicate::Position(1)])],
        outer: &[],
    },
    Case {
        xpath: "//*/following-sibling::a",
        steps: &[
            DOUBLE_SLASH,
            Step::Axis(Axis::Child, Test::AnyElement, &[]),
            Step::Axis(Axis::FollowingSibling, Test::Name('a'), &[]),
        ],
        outer: &[],
    },
    Case {
        xpath: "//a/following::b",
        steps: &[DOUBLE_SLASH, CHILD_A, Step::Axis(Axis::Following, Test::Name('b'), &[])],
        outer: &[],
    },
    Case {
        xpath: "//a/following::b[1]",
        steps: &[DOUBLE_SLASH, CHILD_A, Step::Axis(Axis::Following, Test::Name('b'), &[Predicate::Position(1)])],
        outer: &[],
    },
    Case {
        xpath: "//a/preceding::b[1]",
        steps: &[DOUBLE_SLASH, CHILD_A, Step::Axis(Axis::Preceding, Test::Name('b'), &[Predicate::Position(1)])],
        outer: &[],
    },
    Case {
        xpath: "//a/ancestor::*[1]",
        steps: &[DOUBLE_SLASH, CHILD_A, Step::Axis(Axis::Ancestor, Test::AnyElement, &[Predicate::Position(1)])],
        outer: &[],
    },
    Case {
        xpath: "//a/descendant::b[1]",
        steps: &[DOUBLE_SLASH, CHILD_A, Step::Axis(Axis::Descendant, Test::Name('b'), &[Predicate::Position(1)])],
        outer: &[],
    },
    Case {
        xpath: "//(a|b)[1]",
        steps: &[
            DOUBLE_SLASH,
            Step::Union(&[(Axis::Child, Test::Name('a')), (Axis::Child, Test::Name('b'))], &[Predicate::Position(1)]),
        ],
        outer: &[],
    },
    Case {
        xpath: "//(a|b)[b]",
        steps: &[
            DOUBLE_SLASH,
            Step::Union(&[(Axis::Child, Test::Name('a')), (Axis::Child, Test::Name('b'))], &[Predicate::HasChildB]),
        ],
        outer: &[],
    },
    Case {
        xpath: "//*/..",
        steps: &[
            DOUBLE_SLASH,
            Step::Axis(Axis::Child, Test::AnyElement, &[]),
            Step::Axis(Axis::Parent, Test::AnyNode, &[]),
        ],
        outer: &[],
    },
];

#[derive(Clone)]
struct Tree {
    name: char,
    children: Vec<Tree>,
}

fn trees(size: usize) -> Vec<Tree> {
    let mut out = Vec::new();
    for children in forests(size - 1) {
        for name in ['a', 'b'] {
            out.push(Tree { name, children: children.clone() });
        }
    }
    out
}

fn forests(size: usize) -> Vec<Vec<Tree>> {
    if size == 0 {
        return vec![Vec::new()];
    }
    let mut out = Vec::new();
    for first in 1..=size {
        for head in trees(first) {
            for tail in forests(size - first) {
                let mut forest = vec![head.clone()];
                forest.extend(tail);
                out.push(forest);
            }
        }
    }
    out
}

/// The bracket notation of `tree`, each label its name plus its pre-order index from 1.
fn notation(tree: &Tree, next: &mut usize) -> String {
    *next += 1;
    let mut out = format!("{}{next}", tree.name);
    if !tree.children.is_empty() {
        let children: Vec<String> = tree.children.iter().map(|child| notation(child, next)).collect();
        out.push_str(":[");
        out.push_str(&children.join(","));
        out.push(']');
    }
    out
}

/// A tree as pre-order indices: 0 is the document, the root element is 1.
struct Naive {
    parent: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    name: Vec<Option<char>>,
    end: Vec<usize>,
}

impl Naive {
    fn new(tree: &Tree) -> Self {
        fn add(naive: &mut Naive, tree: &Tree, parent: usize) {
            let index = naive.name.len();
            naive.parent.push(Some(parent));
            naive.children.push(Vec::new());
            naive.name.push(Some(tree.name));
            naive.end.push(index);
            naive.children[parent].push(index);
            for child in &tree.children {
                add(naive, child, index);
            }
            naive.end[index] = naive.name.len() - 1;
        }
        let mut naive = Naive { parent: vec![None], children: vec![Vec::new()], name: vec![None], end: vec![0] };
        add(&mut naive, tree, 0);
        naive.end[0] = naive.name.len() - 1;
        naive
    }

    fn is_ancestor(&self, ancestor: usize, node: usize) -> bool {
        ancestor < node && node <= self.end[ancestor]
    }

    /// The axis from `node` in axis order: document order forward, nearest first in reverse.
    fn axis(&self, node: usize, axis: Axis) -> Vec<usize> {
        let siblings = || self.parent[node].map(|p| self.children[p].clone()).unwrap_or_default();
        match axis {
            Axis::Child => self.children[node].clone(),
            Axis::Descendant => (node + 1..=self.end[node]).collect(),
            Axis::DescendantOrSelf => (node..=self.end[node]).collect(),
            Axis::Parent => self.parent[node].into_iter().collect(),
            Axis::Ancestor => {
                let mut out = Vec::new();
                let mut current = self.parent[node];
                while let Some(parent) = current {
                    out.push(parent);
                    current = self.parent[parent];
                }
                out
            }
            Axis::FollowingSibling => siblings().into_iter().filter(|&s| s > node).collect(),
            Axis::Following => (self.end[node] + 1..self.name.len()).collect(),
            Axis::Preceding => (0..node).rev().filter(|&n| !self.is_ancestor(n, node)).collect(),
        }
    }

    fn passes(&self, node: usize, test: Test) -> bool {
        match test {
            Test::Name(name) => self.name[node] == Some(name),
            Test::AnyElement => self.name[node].is_some(),
            Test::AnyNode => true,
        }
    }

    fn apply(&self, list: Vec<usize>, predicates: &[Predicate]) -> Vec<usize> {
        predicates.iter().fold(list, |list, predicate| match predicate {
            Predicate::Position(n) => list.get(n - 1).copied().into_iter().collect(),
            Predicate::Last => list.last().copied().into_iter().collect(),
            Predicate::HasChildB => {
                list.into_iter().filter(|&n| self.children[n].iter().any(|&c| self.name[c] == Some('b'))).collect()
            }
        })
    }

    fn evaluate(&self, case: &Case) -> Vec<String> {
        let mut current = vec![0];
        for step in case.steps {
            let mut next = Vec::new();
            for &context in &current {
                let selected = match step {
                    Step::Axis(axis, test, predicates) => {
                        let list = self.axis(context, *axis).into_iter().filter(|&n| self.passes(n, *test)).collect();
                        self.apply(list, predicates)
                    }
                    Step::Union(parts, predicates) => {
                        let mut list: Vec<usize> = parts
                            .iter()
                            .flat_map(|(axis, test)| {
                                self.axis(context, *axis).into_iter().filter(|&n| self.passes(n, *test))
                            })
                            .collect();
                        list.sort_unstable();
                        list.dedup();
                        self.apply(list, predicates)
                    }
                };
                next.extend(selected);
            }
            next.sort_unstable();
            next.dedup();
            current = next;
        }
        self.apply(current, case.outer)
            .into_iter()
            .map(|n| self.name[n].map_or_else(|| "#doc".to_string(), |name| format!("{name}{n}")))
            .collect()
    }
}

fn run<N: 'static + XdmNode>(
    program: &CompiledXPath,
    node: &N,
    functions: &Rc<FunctionImplementations<N>>,
    what: &str,
) -> Vec<String> {
    let ctx = DynamicContextBuilder::default()
        .with_context_item(XdmItem::Node(node.clone()))
        .with_functions(Rc::clone(functions))
        .build();
    labels(&evaluate(program, &ctx).unwrap_or_else(|e| panic!("{what}: {e:?}")))
}

#[test]
fn the_engine_agrees_with_a_naive_evaluator_on_every_small_tree() {
    let programs: Vec<(&Case, CompiledXPath)> =
        CASES.iter().map(|case| (case, compile(case.xpath).expect("the case compiles"))).collect();
    let keyed_functions = default_function_registry::<SimpleNode>();
    let keyless_functions = default_function_registry::<Keyless>();
    let mut checked = 0;
    for size in 1..=6 {
        for shape in trees(size) {
            let notation = notation(&shape, &mut 0);
            let document = tree(&notation);
            let keyless = Keyless::new(&document);
            let naive = Naive::new(&shape);
            for (case, program) in &programs {
                let expected = naive.evaluate(case);
                let what = format!("`{}` on {notation}", case.xpath);
                let keyed = run(program, &document, &keyed_functions, &what);
                assert_eq!(keyed, expected, "keyed model: {what}");
                let keyless = run(program, &keyless, &keyless_functions, &what);
                assert_eq!(keyless, expected, "keyless model: {what}");
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 3238 * CASES.len(), "every tree of up to six elements, with every case");
}

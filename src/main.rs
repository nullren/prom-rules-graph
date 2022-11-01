use clap::Parser;
use multimap::MultiMap;
use promql::Node;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;

/// Create a graph of metric dependencies from Prometheus recording rules.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Prometheus server endpoint
    #[clap(short, long, value_parser, default_value = "http://localhost:9090")]
    prom_endpoint: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rules {
    pub status: String,
    pub data: Data,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub groups: Vec<Group>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub name: String,
    pub file: String,
    pub rules: Vec<Rule>,
    pub interval: i64,
    pub limit: i64,
    pub evaluation_time: f64,
    pub last_evaluation: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub name: String,
    pub query: String,
    pub health: String,
    pub evaluation_time: f64,
    pub last_evaluation: String,
    #[serde(rename = "type")]
    pub type_field: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();

    let rules_url = format!("{}/api/v1/rules", args.prom_endpoint);
    let resp = reqwest::get(rules_url).await?.text().await?;
    let rules: Rules = serde_json::from_str(&resp)?;

    let mut graph = MultiMap::new();
    let mut eval_times = BTreeMap::new();

    rules
        .data
        .groups
        .iter()
        .flat_map(|g| g.rules.iter())
        .for_each(|r| {
            eval_times.insert(r.name.clone(), r.evaluation_time);
            let node = promql::parse((&r.query).as_ref(), false).unwrap();
            let deps = get_metic_dependencies(node);
            for dep in deps {
                graph.insert(dep.clone(), r.name.clone());
            }
        });

    // dump graph in dot format
    print_dot_digraph(&graph);
    // println!("{:#?}", eval_times);

    Ok(())
}

fn print_dot_digraph(graph: &MultiMap<String, String>) {
    println!("digraph {{");
    println!("  rankdir=\"LR\";");
    for (k, v) in graph.iter_all() {
        for vv in v {
            println!("  \"{}\" -> \"{}\";", k, vv);
        }
    }
    println!("}}");
}

fn get_metic_dependencies(n: Node) -> Vec<String> {
    let mut node_queue = vec![n];
    let mut metrics = vec![];

    while node_queue.len() > 0 {
        let node = node_queue.pop().unwrap();
        match node {
            Node::Operator { x, op: _op, y } => {
                node_queue.push(*x);
                node_queue.push(*y);
            }
            Node::Vector(v) => {
                let labels = v.labels;
                for lm in labels {
                    if lm.name == "__name__" {
                        metrics.push(lm.value);
                    }
                }
            }
            Node::Scalar(_s) => {}
            Node::String(_s) => {}
            Node::Function {
                name: _name,
                aggregation: _aggregation,
                args,
            } => {
                for arg in args {
                    node_queue.push(arg);
                }
            }
            Node::Negation(n) => {
                node_queue.push(*n);
            }
        }
    }
    metrics
}

#[cfg(test)]
mod tests {
    use super::*;
    use promql::parse;

    #[test]
    fn test_get_metic_dependencies() {
        let ast = parse(
            b"
        sum(1 - something_used{env=\"production\"} / something_total) by (instance)
        and ignoring (instance)
        sum(rate(some_queries{instance=~\"localhost\\\\d+\"} [5m])) > 100
    ",
            false,
        )
        .unwrap(); // or show user that their query is invalid

        let metrics = get_metic_dependencies(ast);
        assert_eq!(
            metrics,
            vec!["some_queries", "something_total", "something_used"]
        );
    }
}

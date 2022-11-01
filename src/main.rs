use promql::Node;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let resp = reqwest::get("https://httpbin.org/ip").await?.text().await?;
    println!("{:#?}", resp);
    Ok(())
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

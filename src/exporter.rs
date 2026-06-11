use anyhow::Result;
use std::fs::File;
use std::io::Write;

pub fn generate_html(db_path: &str, out_path: &str, filter_path: &str) -> Result<()> {
    let conn = crate::open_db_connection(db_path).map_err(|e| anyhow::anyhow!(e))?;

    let mut elements = Vec::new();
    let mut included_nodes = std::collections::HashSet::new();

    let node_query = "MATCH (n) RETURN n.id AS id, label(n) as label, n.name AS name, n.kind AS kind";
    let mut nodes_res = conn.query(node_query).map_err(|e| anyhow::anyhow!(e))?;
    let cols = nodes_res.get_column_names();

    for row in nodes_res.by_ref() {
        let get_val = |name: &str| -> String {
            cols.iter().position(|c| c == name).and_then(|idx| {
                if let lbug::Value::String(s) = &row[idx] { Some(s.clone()) } else { None }
            }).unwrap_or_default()
        };
        let id = get_val("id");
        if id.is_empty() { continue; }

        let label = get_val("label");
        let name = get_val("name");
        let kind = get_val("kind");

        if filter_path != "." && !id.contains(filter_path) {
            continue;
        }

        included_nodes.insert(id.clone());

        let color = match kind.as_str() {
            "Method" | "Function" => "#a6e22e",
            "Class" | "Struct" | "Interface" => "#f92672",
            "Module" | "Namespace" => "#66d9ef",
            "Field" | "Property" => "#fd971f",
            _ => {
                if label == "File" { "#ae81ff" } else { "#ffffff" }
            }
        };

        let node_label = if name.is_empty() {
            id.split('/').last().unwrap_or(&id).to_string()
        } else {
            name.clone()
        };

        elements.push(format!(
            "{{ data: {{ id: '{id}', label: '{node_label}', color: '{color}' }}, classes: '{kind}' }}",
        ));
    }

    let edge_query = "MATCH (s)-[r]->(t) RETURN s.id AS s_id, t.id AS t_id, struct_extract(r, '_LABEL') as edge_label";
    let mut edges_res = conn.query(edge_query).map_err(|e| anyhow::anyhow!(e))?;
    let e_cols = edges_res.get_column_names();

    for row in edges_res.by_ref() {
        let get_val = |name: &str| -> String {
            e_cols.iter().position(|c| c == name).and_then(|idx| {
                if let lbug::Value::String(s) = &row[idx] { Some(s.clone()) } else { None }
            }).unwrap_or_default()
        };
        let source = get_val("s_id");
        let target = get_val("t_id");
        let edge_label = get_val("edge_label");

        if !included_nodes.contains(&source) || !included_nodes.contains(&target) {
            continue;
        }

        elements.push(format!(
            "{{ data: {{ id: '{source}-{target}', source: '{source}', target: '{target}', label: '{edge_label}' }} }}",
        ));
    }

    let elements_json = format!("[{}]", elements.join(",\n"));

    let html_content = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>icnow - Code Graph</title>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/cytoscape/3.26.0/cytoscape.min.js"></script>
    ...
    <script>
        var elements = {elements_json};
        // cytoscape setup here
    </script>
</body>
</html>"#
    );

    let mut file = File::create(out_path)?;
    file.write_all(html_content.as_bytes())?;

    Ok(())
}

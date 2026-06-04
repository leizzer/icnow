use anyhow::{Context, Result};
use serde_json::json;
use std::fs::File;
use std::io::Write;

pub fn generate_html(db_path: &str, out_path: &str, filter_path: &str) -> Result<()> {
    let conn = crate::open_db_connection(db_path).context("Failed to open db")?;

    let mut elements = Vec::new();
    let mut included_nodes = std::collections::HashSet::new();

    let node_query = "MATCH (n) RETURN n.id, labels(n) as label, n.name, n.kind";
    let nodes_res = conn.cypher(node_query).context("Query nodes failed")?;

    for row in &nodes_res {
        let id: String = row.get("n.id").unwrap_or_else(|_| "unknown".to_string());

        if !filter_path.is_empty() && !id.starts_with(filter_path) {
            continue;
        }

        included_nodes.insert(id.clone());

        let mut label: String = row.get("label").unwrap_or_else(|_| "Node".to_string());
        label = label
            .replace("[\"", "")
            .replace("\"]", "")
            .replace("\"", "");
        let name: String = row.get("n.name").unwrap_or_else(|_| id.clone());
        let kind: String = row.get("n.kind").unwrap_or_else(|_| "".to_string());

        let display_name = if label == "File" {
            std::path::Path::new(&id)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        } else if !name.is_empty() {
            name
        } else {
            id.clone()
        };

        elements.push(json!({
            "data": {
                "id": id,
                "label": label,
                "name": display_name,
                "kind": kind
            }
        }));
    }

    let edge_query = "MATCH (s)-[r]->(t) RETURN s.id, t.id, type(r) as edge_label";
    let edges_res = conn.cypher(edge_query).context("Query edges failed")?;

    for row in &edges_res {
        let source: String = row.get("s.id").unwrap_or_default();
        let target: String = row.get("t.id").unwrap_or_default();

        if !filter_path.is_empty()
            && (!included_nodes.contains(&source) || !included_nodes.contains(&target))
        {
            continue;
        }

        let mut edge_label: String = row.get("edge_label").unwrap_or_default();

        if edge_label.starts_with("REL_") {
            edge_label = edge_label.replace("REL_", "");
        }

        if !source.is_empty() && !target.is_empty() {
            let edge_id = format!("{source}::{edge_label}::{target}");
            elements.push(json!({
                "data": {
                    "id": edge_id,
                    "source": source,
                    "target": target,
                    "label": edge_label
                }
            }));
        }
    }

    let json_data = serde_json::to_string(&elements)?;

    let mut file = File::create(out_path)?;

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>icnow - Interactive Graph</title>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/cytoscape/3.28.1/cytoscape.min.js"></script>
    <script src="https://unpkg.com/layout-base/layout-base.js"></script>
    <script src="https://unpkg.com/cose-base/cose-base.js"></script>
    <script src="https://unpkg.com/cytoscape-fcose/cytoscape-fcose.js"></script>
    <style>
        body {{ margin: 0; padding: 0; background-color: #121212; color: #ffffff; font-family: 'Segoe UI', sans-serif; overflow: hidden; }}
        #cy {{ width: 100vw; height: 100vh; display: block; }}
        #panel {{ position: absolute; top: 20px; left: 20px; background: rgba(30, 30, 30, 0.9); padding: 15px 25px; border-radius: 12px; border: 1px solid #333; z-index: 1000; }}
        h1 {{ margin: 0 0 5px 0; font-size: 1.5rem; color: #4DB8FF; }}
        p {{ margin: 0; font-size: 0.9rem; color: #AAAAAA; }}
        .legend {{ margin-top: 15px; font-size: 0.85rem; }}
        .legend-item {{ display: flex; align-items: center; margin-bottom: 5px; }}
        .legend-color {{ width: 12px; height: 12px; border-radius: 3px; margin-right: 8px; }}
    </style>
</head>
<body>
    <div id="panel">
        <h1>icnow Knowledge Graph</h1>
        <p>Interactive Architecture Explorer</p>
        <div class="legend">
            <div class="legend-item"><div class="legend-color" style="background:#4DB8FF;"></div>File</div>
            <div class="legend-item"><div class="legend-color" style="background:#FF6B6B;"></div>Function</div>
            <div class="legend-item"><div class="legend-color" style="background:#4CAF50;"></div>Class</div>
            <div class="legend-item"><div class="legend-color" style="background:#9C27B0;"></div>Import/Other</div>
        </div>
    </div>
    <div id="cy"></div>
    <script>
        window.GRAPH_ELEMENTS = {json_data};
        document.addEventListener('DOMContentLoaded', function() {{
            var cy = cytoscape({{
                container: document.getElementById('cy'),
                elements: window.GRAPH_ELEMENTS,
                style: [
                    {{ selector: 'node', style: {{ 'label': 'data(name)', 'color': '#ffffff', 'text-outline-color': '#222', 'text-outline-width': 2, 'font-size': '10px', 'background-color': '#9C27B0', 'width': 25, 'height': 25 }} }},
                    {{ selector: 'node[label = "File"]', style: {{ 'background-color': '#4DB8FF', 'shape': 'round-rectangle', 'width': 35, 'height': 35 }} }},
                    {{ selector: 'node[label = "Function"]', style: {{ 'background-color': '#FF6B6B', 'shape': 'ellipse' }} }},
                    {{ selector: 'node[label = "Method"]', style: {{ 'background-color': '#FF9800', 'shape': 'ellipse' }} }},
                    {{ selector: 'node[label = "Class"]', style: {{ 'background-color': '#4CAF50', 'shape': 'hexagon', 'width': 35, 'height': 30 }} }},
                    {{ selector: 'edge', style: {{ 'width': 1.5, 'line-color': '#555', 'target-arrow-color': '#555', 'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'opacity': 0.6, 'label': 'data(label)', 'font-size': '8px', 'color': '#888', 'text-rotation': 'autorotate', 'text-background-opacity': 1, 'text-background-color': '#121212' }} }},
                    {{ selector: 'edge[label = "CONTAINS"]', style: {{ 'line-color': '#4DB8FF', 'target-arrow-color': '#4DB8FF', 'opacity': 0.3, 'width': 1, 'line-style': 'dotted' }} }},
                    {{ selector: 'edge[label = "CALLS"]', style: {{ 'line-color': '#FF6B6B', 'target-arrow-color': '#FF6B6B', 'width': 2 }} }},
                    {{ selector: 'edge[label = "IMPORTS"]', style: {{ 'line-color': '#4CAF50', 'target-arrow-color': '#4CAF50', 'width': 2, 'line-style': 'dashed' }} }}
                ],
                layout: {{ name: 'fcose', quality: 'default', randomize: true, animate: true, fit: true, padding: 50, nodeDimensionsIncludeLabels: true }}
            }});
            cy.on('tap', 'node', function(evt){{ var node = evt.target; cy.elements().style({{ 'opacity': 0.1 }}); node.neighborhood().add(node).style({{ 'opacity': 1 }}); }});
            cy.on('tap', function(evt){{ if(evt.target === cy){{ cy.elements().style({{ 'opacity': 1 }}); }} }});
        }});
    </script>
</body>
</html>"#
    );

    file.write_all(html.as_bytes())?;
    Ok(())
}

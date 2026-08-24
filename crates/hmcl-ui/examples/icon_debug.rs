//! Diagnostic: check that icon paths parse and tessellate into visible geometry.
use hmcl_ui::widgets::icon;

fn main() {
    for name in [
        "FORT",
        "CLOSE",
        "MINIMIZE_CENTER",
        "HELP",
        "PERSON",
        "SETTINGS",
        "DOWNLOAD",
        "ADD",
        "ADD_CIRCLE",
        "CHECK_CIRCLE",
    ] {
        let shapes = icon::icon_shapes(
            name,
            egui::Pos2::new(10.0, 10.0),
            20.0,
            egui::Color32::WHITE,
        );
        match shapes {
            Some(shapes) => {
                let mut total_vertices = 0usize;
                for shape in &shapes {
                    if let egui::Shape::Mesh(mesh) = shape {
                        total_vertices += mesh.vertices.len();
                    }
                }
                println!("{name}: mesh with {total_vertices} vertices");
            }
            None => println!("{name}: NO SHAPES"),
        }
    }
}

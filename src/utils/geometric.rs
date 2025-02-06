use nalgebra::{Unit, Vector3};

pub fn axis_angle_for_b2a(a: Vector3<f64>, b: Vector3<f64>) -> (Unit<Vector3<f64>>, f64) {
    let axis = b.cross(&a);
    let axis = Unit::new_normalize(if axis.norm() == 0. {
        if a.cross(&Vector3::x()).norm() == 0. {
            Vector3::y()
        } else {
            Vector3::x()
        }
    } else {
        axis
    });
    let angle = (b.dot(&a) / (a.norm() * b.norm())).acos();
    let angle = if angle.is_nan() { 0. } else { angle };
    (axis, angle)
}

#[test]
fn reverse_vectors() {
    println!(
        "{:#?}",
        axis_angle_for_b2a(Vector3::new(1., 0., 0.), Vector3::new(-2., 0., 0.))
    )
}

#[test]
fn same_direction() {
    println!(
        "{:#?}",
        axis_angle_for_b2a(Vector3::new(1., 0., 0.), Vector3::new(-4.5, 1e-8, 1e-16))
    )
}

#[test]
fn vertical_vectors() {
    println!(
        "{:#?}",
        axis_angle_for_b2a(Vector3::new(-1., 0., 0.), Vector3::new(0., 2., 0.))
    )
}

#[test]
fn zero_vector() {
    println!(
        "{:#?}",
        axis_angle_for_b2a(Vector3::new(1., 0., 0.), Vector3::new(0., 0., 0.))
    )
}

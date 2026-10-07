//! The few value forms a Quad definition uses beyond a single number or
//! labelled parts (#16 §3): a box, a number "at" another, and a curve of
//! points. Each is read with the shared unit list.

use crate::units::{self, Quantity, UnitProblem};

/// Reads `"box 64 × 10 × 6 mm"`: three lengths, front to back, side to side
/// and top to bottom. `x` reads the same as `×`, and the unit may be written
/// once, at the end.
pub fn parse_box(text: &str) -> Result<[Quantity; 3], UnitProblem> {
    let trimmed = text.trim();
    let Some(rest) = trimmed.strip_prefix("box ") else {
        return Err(UnitProblem(format!(
            "\"{trimmed}\" isn't a box: write \"box\" and its three sizes, front to back, side to side and top to bottom, such as \"box 64 × 10 × 6 mm\""
        )));
    };
    let sides: Vec<&str> = rest.split(['×', 'x']).collect();
    if sides.len() != 3 {
        return Err(UnitProblem(format!(
            "\"{trimmed}\" needs three sizes joined by ×, such as \"box 64 × 10 × 6 mm\""
        )));
    }
    let mut quantities = Vec::new();
    for side in &sides {
        quantities.push(units::parse_quantity(side)?);
    }
    let last = quantities[2].unit.clone();
    for quantity in &mut quantities[..2] {
        if quantity.unit.dimension() == units::Dimension::NONE {
            quantity.value *= last.in_si();
            quantity.unit = last.clone();
        }
    }
    let [a, b, c]: [Quantity; 3] = quantities
        .try_into()
        .map_err(|_| UnitProblem("a box has three sizes".to_string()))?;
    Ok([a, b, c])
}

/// Reads `"1.2 A at 10 V"`: a number, and the condition it was measured at.
pub fn parse_at(text: &str) -> Result<(Quantity, Quantity), UnitProblem> {
    let trimmed = text.trim();
    let Some((value, condition)) = trimmed.split_once(" at ") else {
        return Err(UnitProblem(format!(
            "\"{trimmed}\" needs the condition it holds at, such as \"1.2 A at 10 V\""
        )));
    };
    Ok((
        units::parse_quantity(value)?,
        units::parse_quantity(condition)?,
    ))
}

/// Reads a curve written as one line of points, such as
/// `"4.35 V at 100%, 3.92 V at 50%, 3.30 V at 0%"`: each point is a value
/// "at" where it holds.
pub fn parse_curve(text: &str) -> Result<Vec<(Quantity, Quantity)>, UnitProblem> {
    units::refuse_commas_in_numbers(text)?;
    let trimmed = text.trim();
    let mut points = Vec::new();
    for point in trimmed.split(',') {
        points.push(parse_at(point)?);
    }
    if points.len() < 2 {
        return Err(UnitProblem(format!(
            "\"{trimmed}\" is a curve, so it needs at least two points, such as \"4.35 V at 100%, 3.30 V at 0%\""
        )));
    }
    Ok(points)
}

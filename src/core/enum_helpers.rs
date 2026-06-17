//! Manual ports of methods on the generated option enums
//! (`PortSide`, `Direction`, `PortConstraints`, ...).

use crate::core::options_gen::{
    Direction, NodeLabelPlacement, PortConstraints, PortLabelPlacement, PortSide, SizeConstraint,
};
use crate::graph::properties::EnumSet;

impl PortSide {
    /// Side after this one in clockwise order.
    pub fn right(self) -> PortSide {
        match self {
            PortSide::NORTH => PortSide::EAST,
            PortSide::EAST => PortSide::SOUTH,
            PortSide::SOUTH => PortSide::WEST,
            PortSide::WEST => PortSide::NORTH,
            PortSide::UNDEFINED => PortSide::UNDEFINED,
        }
    }

    /// Side before this one in clockwise order.
    pub fn left(self) -> PortSide {
        match self {
            PortSide::NORTH => PortSide::WEST,
            PortSide::EAST => PortSide::NORTH,
            PortSide::SOUTH => PortSide::EAST,
            PortSide::WEST => PortSide::SOUTH,
            PortSide::UNDEFINED => PortSide::UNDEFINED,
        }
    }

    pub fn opposed(self) -> PortSide {
        match self {
            PortSide::NORTH => PortSide::SOUTH,
            PortSide::EAST => PortSide::WEST,
            PortSide::SOUTH => PortSide::NORTH,
            PortSide::WEST => PortSide::EAST,
            PortSide::UNDEFINED => PortSide::UNDEFINED,
        }
    }

    pub fn are_adjacent(self, other: PortSide) -> bool {
        if self == PortSide::UNDEFINED {
            false
        } else {
            self.left() == other || self.right() == other
        }
    }

    pub fn from_direction(direction: Direction) -> PortSide {
        match direction {
            Direction::UP => PortSide::NORTH,
            Direction::RIGHT => PortSide::EAST,
            Direction::DOWN => PortSide::SOUTH,
            Direction::LEFT => PortSide::WEST,
            Direction::UNDEFINED => PortSide::UNDEFINED,
        }
    }

    pub fn is_vertical(side: PortSide) -> bool {
        side == PortSide::NORTH || side == PortSide::SOUTH
    }

    pub fn is_horizontal(side: PortSide) -> bool {
        side == PortSide::WEST || side == PortSide::EAST
    }
}

impl Direction {
    pub fn is_horizontal(self) -> bool {
        self == Direction::LEFT || self == Direction::RIGHT
    }

    pub fn is_vertical(self) -> bool {
        self == Direction::UP || self == Direction::DOWN
    }

    pub fn opposite(self) -> Direction {
        match self {
            Direction::LEFT => Direction::RIGHT,
            Direction::RIGHT => Direction::LEFT,
            Direction::UP => Direction::DOWN,
            Direction::DOWN => Direction::UP,
            Direction::UNDEFINED => Direction::UNDEFINED,
        }
    }
}

impl PortConstraints {
    pub fn is_pos_fixed(self) -> bool {
        self == PortConstraints::FIXED_POS
    }

    pub fn is_ratio_fixed(self) -> bool {
        self == PortConstraints::FIXED_RATIO
    }

    pub fn is_order_fixed(self) -> bool {
        matches!(
            self,
            PortConstraints::FIXED_ORDER | PortConstraints::FIXED_RATIO | PortConstraints::FIXED_POS
        )
    }

    pub fn is_side_fixed(self) -> bool {
        self != PortConstraints::FREE && self != PortConstraints::UNDEFINED
    }
}

impl PortLabelPlacement {
    pub fn inside() -> EnumSet<PortLabelPlacement> {
        EnumSet::of(&[PortLabelPlacement::INSIDE])
    }

    pub fn outside() -> EnumSet<PortLabelPlacement> {
        EnumSet::of(&[PortLabelPlacement::OUTSIDE])
    }

    /// Neither INSIDE nor OUTSIDE
    /// is included.
    pub fn is_fixed(placement: EnumSet<PortLabelPlacement>) -> bool {
        !placement.contains(PortLabelPlacement::INSIDE)
            && !placement.contains(PortLabelPlacement::OUTSIDE)
    }

    pub fn is_valid(placement: EnumSet<PortLabelPlacement>) -> bool {
        let inside_outside = [PortLabelPlacement::INSIDE, PortLabelPlacement::OUTSIDE];
        if inside_outside.iter().filter(|&&v| placement.contains(v)).count() > 1 {
            return false;
        }
        let position = [
            PortLabelPlacement::ALWAYS_SAME_SIDE,
            PortLabelPlacement::ALWAYS_OTHER_SAME_SIDE,
            PortLabelPlacement::SPACE_EFFICIENT,
        ];
        if position.iter().filter(|&&v| placement.contains(v)).count() > 1 {
            return false;
        }
        true
    }
}

impl NodeLabelPlacement {
    pub fn is_valid(placement: EnumSet<NodeLabelPlacement>) -> bool {
        let inside_outside = [NodeLabelPlacement::INSIDE, NodeLabelPlacement::OUTSIDE];
        if inside_outside.iter().filter(|&&v| placement.contains(v)).count() > 1 {
            return false;
        }
        let horizontal = [
            NodeLabelPlacement::H_LEFT,
            NodeLabelPlacement::H_CENTER,
            NodeLabelPlacement::H_RIGHT,
        ];
        if horizontal.iter().filter(|&&v| placement.contains(v)).count() > 1 {
            return false;
        }
        let vertical = [
            NodeLabelPlacement::V_TOP,
            NodeLabelPlacement::V_CENTER,
            NodeLabelPlacement::V_BOTTOM,
        ];
        if vertical.iter().filter(|&&v| placement.contains(v)).count() > 1 {
            return false;
        }
        true
    }
}

impl SizeConstraint {
    /// Empty set.
    pub fn fixed() -> EnumSet<SizeConstraint> {
        EnumSet::none()
    }

    pub fn free() -> EnumSet<SizeConstraint> {
        EnumSet::of(&[
            SizeConstraint::PORTS,
            SizeConstraint::PORT_LABELS,
            SizeConstraint::NODE_LABELS,
        ])
    }
}

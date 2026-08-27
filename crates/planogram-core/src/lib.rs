use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::ops::{Add, Sub};

#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Length(i32);

impl Length {
    pub const ZERO: Self = Self(0);

    pub const fn from_sixteenths(value: i32) -> Self {
        Self(value)
    }

    pub const fn sixteenths(self) -> i32 {
        self.0
    }

    pub const fn inches(inches: i32) -> Self {
        Self(inches * 16)
    }

    pub const fn feet(feet: i32) -> Self {
        Self(feet * 12 * 16)
    }
}

impl Add for Length {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Length {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }
        }
    };
}

id_type!(FixtureId);
id_type!(SectionId);
id_type!(ShelfId);
id_type!(VersionId);
id_type!(ChangeSetId);
id_type!(ProductId);
id_type!(PlacementId);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShelfKind {
    BaseDeck,
    Adjustable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Shelf {
    pub id: ShelfId,
    pub section_id: SectionId,
    pub kind: ShelfKind,
    pub width: Length,
    pub depth: Length,
    pub elevation: Length,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub id: SectionId,
    pub fixture_id: FixtureId,
    pub sequence: u32,
    pub width: Length,
    pub height: Length,
    pub shelves: Vec<Shelf>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Fixture {
    pub id: FixtureId,
    pub name: String,
    pub width: Length,
    pub height: Length,
    pub depth: Length,
    pub sections: Vec<Section>,
}

pub const DEFAULT_FIXTURE_WIDTH: Length = Length::from_sixteenths(768);
pub const DEFAULT_FIXTURE_HEIGHT: Length = Length::from_sixteenths(1_344);
pub const DEFAULT_FIXTURE_DEPTH: Length = Length::from_sixteenths(352);
pub const DEFAULT_BASE_DECK_DEPTH: Length = Length::from_sixteenths(352);
pub const DEFAULT_ADJUSTABLE_SHELF_DEPTH: Length = Length::from_sixteenths(256);
pub const MIN_PLACEMENT_GAP: Length = Length::from_sixteenths(2);
pub const DEFAULT_SHELF_ELEVATIONS: [Length; 6] = [
    Length::from_sixteenths(192),
    Length::from_sixteenths(384),
    Length::from_sixteenths(576),
    Length::from_sixteenths(768),
    Length::from_sixteenths(960),
    Length::from_sixteenths(1_152),
];

pub fn default_fixture() -> Fixture {
    let fixture_id = FixtureId::new("fixture_standard_4ft");
    let section_id = SectionId::new("section_01");
    let mut shelves = vec![Shelf {
        id: ShelfId::new("base_deck"),
        section_id: section_id.clone(),
        kind: ShelfKind::BaseDeck,
        width: DEFAULT_FIXTURE_WIDTH,
        depth: DEFAULT_BASE_DECK_DEPTH,
        elevation: Length::ZERO,
    }];
    shelves.extend(
        DEFAULT_SHELF_ELEVATIONS
            .iter()
            .enumerate()
            .map(|(index, elevation)| Shelf {
                id: ShelfId::new(format!("shelf_{:02}", index + 1)),
                section_id: section_id.clone(),
                kind: ShelfKind::Adjustable,
                width: DEFAULT_FIXTURE_WIDTH,
                depth: DEFAULT_ADJUSTABLE_SHELF_DEPTH,
                elevation: *elevation,
            }),
    );

    Fixture {
        id: fixture_id.clone(),
        name: "4' Standard Bay".into(),
        width: DEFAULT_FIXTURE_WIDTH,
        height: DEFAULT_FIXTURE_HEIGHT,
        depth: DEFAULT_FIXTURE_DEPTH,
        sections: vec![Section {
            id: section_id,
            fixture_id,
            sequence: 0,
            width: DEFAULT_FIXTURE_WIDTH,
            height: DEFAULT_FIXTURE_HEIGHT,
            shelves,
        }],
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionStatus {
    Draft,
    Proposed,
    Published,
    Archived,
}

impl VersionStatus {
    fn is_editable(self) -> bool {
        matches!(self, Self::Draft | Self::Proposed)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MoveShelf {
    pub shelf_id: ShelfId,
    pub before: Length,
    pub after: Length,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlacementLocation {
    pub shelf_id: ShelfId,
    pub x: Length,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MovePlacement {
    pub placement_id: PlacementId,
    pub before: PlacementLocation,
    pub after: PlacementLocation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductDimensions {
    pub width: Length,
    pub height: Length,
    pub depth: Length,
    pub source: String,
    pub confidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductPerformance {
    pub sales_per_store_per_week_cents: u64,
    pub units_per_store_per_week_milliunits: u64,
    pub gross_margin_basis_points: i32,
    pub source: String,
    pub period: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrayConfiguration {
    pub facings_x: u32,
    pub units_deep: u32,
    #[serde(rename = "outer_width_sixteenths")]
    pub outer_width: Length,
    #[serde(rename = "outer_height_sixteenths")]
    pub outer_height: Length,
    #[serde(rename = "outer_depth_sixteenths")]
    pub outer_depth: Length,
    #[serde(rename = "front_lip_height_sixteenths")]
    pub front_lip_height: Length,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Product {
    pub id: ProductId,
    pub upc: String,
    pub brand: String,
    pub description: String,
    pub size_oz: String,
    pub category: String,
    pub dimensions: ProductDimensions,
    pub net_weight_ounces_hundredths: u32,
    pub casepack_quantity: u32,
    pub performance: ProductPerformance,
    pub tray: Option<TrayConfiguration>,
    pub color: [u8; 3],
    pub lid_color: [u8; 3],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub id: PlacementId,
    pub product_id: ProductId,
    pub shelf_id: ShelfId,
    pub x: Length,
    pub facings_x: u32,
    pub facings_y: u32,
    pub facings_z: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StockingMode {
    Loose,
    Tray,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlacementGeometry {
    pub display_width: Length,
    pub display_height: Length,
    pub required_depth: Length,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlacementView {
    pub id: PlacementId,
    pub product_id: ProductId,
    pub shelf_id: ShelfId,
    pub x: Length,
    pub stocking_mode: StockingMode,
    pub facings_x: u32,
    pub facings_y: u32,
    pub facings_z: u32,
    pub stocked_unit_count: u32,
    pub geometry: PlacementGeometry,
    pub tray_front_lip_height: Option<Length>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShelfDistribution {
    PackedLeft,
    Centered,
    SpaceBetween,
    SpaceEvenly,
}

/// A model-generated placement intent. The model may choose shelf assignment
/// and sequence; the domain resolves physical coordinates, checks fit/overlap,
/// and applies the complete batch atomically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlacementChange {
    Add {
        /// Only used by an undo proposal. Normal callers leave this unset and
        /// the domain allocates the next stable placement ID during preview.
        placement_id: Option<PlacementId>,
        product_id: ProductId,
        shelf_id: ShelfId,
        /// Zero-based order within the target shelf. The engine resolves this
        /// semantic position to an aligned physical x coordinate.
        sequence: u32,
        /// Internal exact-position override used only when undoing a change
        /// set; WebMCP callers leave this unset.
        resolved_x: Option<Length>,
        facings_x: Option<u32>,
        facings_y: Option<u32>,
        facings_z: Option<u32>,
    },
    Move {
        placement_id: PlacementId,
        shelf_id: ShelfId,
        sequence: u32,
        resolved_x: Option<Length>,
    },
    Remove {
        placement_id: PlacementId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AddPlacement {
    pub placement: Placement,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RemovePlacement {
    pub placement: Placement,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlanogramOperation {
    MoveShelf(MoveShelf),
    MovePlacement(MovePlacement),
    AddPlacement(AddPlacement),
    RemovePlacement(RemovePlacement),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChangeSet {
    pub id: ChangeSetId,
    pub actor: String,
    pub reason: String,
    pub base_revision: u64,
    pub resulting_revision: u64,
    pub operations: Vec<PlanogramOperation>,
    pub compensates: Option<ChangeSetId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationCode {
    VersionNotEditable,
    MissingProduct,
    MissingShelf,
    ShelfNotAdjustable,
    PlacementOnFixedShelf,
    BelowBaseDeck,
    AboveFixture,
    DuplicateElevation,
    ElevationIncrement,
    ProductTooWide,
    ProductTooTall,
    ProductTooDeep,
    NoShelfCapacity,
    PlacementXIncrement,
    PlacementOutOfBounds,
    PlacementOverlap,
    PlacementGap,
    PlacementTooTall,
    PlacementTooDeep,
    InvalidFacingCount,
    TrayFacingMismatch,
    InvalidProductDimensions,
    InvalidNetWeight,
    InvalidCasepackQuantity,
    InvalidProductPerformance,
    InvalidTrayConfiguration,
    DuplicateProductId,
    DuplicateUpc,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub code: ValidationCode,
    pub shelf_id: Option<ShelfId>,
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationSummary {
    pub fn valid(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlanogramValidationResult {
    pub revision: u64,
    pub valid: bool,
    pub validation: ValidationSummary,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ShelfSceneNode {
    pub id: ShelfId,
    pub kind: ShelfKind,
    pub width: Length,
    pub depth: Length,
    pub elevation: Length,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlacementSceneNode {
    pub id: PlacementId,
    pub product_id: ProductId,
    pub shelf_id: ShelfId,
    pub x: Length,
    pub width: Length,
    pub height: Length,
    pub required_depth: Length,
    pub stocking_mode: StockingMode,
    pub stocked_unit_count: u32,
    pub facings_x: u32,
    pub facings_y: u32,
    pub facings_z: u32,
    pub tray_front_lip_height: Option<Length>,
    pub color: [u8; 3],
    pub lid_color: [u8; 3],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenderScene {
    pub revision: u64,
    pub fixture_id: FixtureId,
    pub width: Length,
    pub height: Length,
    pub shelves: Vec<ShelfSceneNode>,
    pub placements: Vec<PlacementSceneNode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScenePatch {
    pub revision: u64,
    pub shelves: Vec<ShelfSceneNode>,
    pub placements: Vec<PlacementSceneNode>,
    pub removed_placement_ids: Vec<PlacementId>,
    pub validation: ValidationSummary,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CommandResult {
    Applied {
        revision: u64,
        change_set: ChangeSet,
        affected_ids: Vec<String>,
        validation: ValidationSummary,
        scene_patch: Box<ScenePatch>,
    },
    ValidationFailed {
        revision: u64,
        validation: ValidationSummary,
    },
    RevisionConflict {
        expected_revision: u64,
        current_revision: u64,
    },
    NotFound {
        entity: String,
        id: String,
    },
    Forbidden {
        message: String,
    },
    InvalidCommand {
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PreviewResult {
    Ready {
        revision: u64,
        operations: Vec<PlanogramOperation>,
        affected_ids: Vec<String>,
        validation: ValidationSummary,
        preview_scene: Box<RenderScene>,
    },
    ValidationFailed {
        revision: u64,
        validation: ValidationSummary,
    },
    RevisionConflict {
        expected_revision: u64,
        current_revision: u64,
    },
    NotFound {
        entity: String,
        id: String,
    },
    Forbidden {
        message: String,
    },
    InvalidCommand {
        message: String,
    },
}

struct PreparedPlacementChanges {
    candidate: DraftVersion,
    operations: Vec<PlanogramOperation>,
    affected_ids: Vec<String>,
    removed_placement_ids: Vec<PlacementId>,
}

enum RequestedPlacementOperation {
    Add(PlacementId),
    Move {
        placement_id: PlacementId,
        before: PlacementLocation,
    },
    Remove(Placement),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DraftVersion {
    pub id: VersionId,
    pub status: VersionStatus,
    pub revision: u64,
    pub fixture: Fixture,
    pub products: Vec<Product>,
    pub placements: Vec<Placement>,
    pub change_sets: Vec<ChangeSet>,
    next_change_set: u64,
    next_placement: u64,
}

impl Default for DraftVersion {
    fn default() -> Self {
        Self {
            id: VersionId::new("version_draft_01"),
            status: VersionStatus::Draft,
            revision: 0,
            fixture: default_fixture(),
            products: default_products(),
            placements: Vec::new(),
            change_sets: Vec::new(),
            next_change_set: 1,
            next_placement: 1,
        }
    }
}

impl DraftVersion {
    pub fn shelf(&self, shelf_id: &ShelfId) -> Option<&Shelf> {
        self.fixture
            .sections
            .iter()
            .flat_map(|section| &section.shelves)
            .find(|shelf| &shelf.id == shelf_id)
    }

    fn shelf_mut(&mut self, shelf_id: &ShelfId) -> Option<&mut Shelf> {
        self.fixture
            .sections
            .iter_mut()
            .flat_map(|section| &mut section.shelves)
            .find(|shelf| &shelf.id == shelf_id)
    }

    pub fn placement(&self, placement_id: &PlacementId) -> Option<&Placement> {
        self.placements
            .iter()
            .find(|placement| &placement.id == placement_id)
    }

    pub fn product(&self, product_id: &ProductId) -> Option<&Product> {
        self.products
            .iter()
            .find(|product| &product.id == product_id)
    }

    fn placement_mut(&mut self, placement_id: &PlacementId) -> Option<&mut Placement> {
        self.placements
            .iter_mut()
            .find(|placement| &placement.id == placement_id)
    }

    fn placement_view_for(placement: &Placement, product: &Product) -> PlacementView {
        let (stocking_mode, geometry, tray_front_lip_height) = if let Some(tray) = &product.tray {
            (
                StockingMode::Tray,
                PlacementGeometry {
                    display_width: tray.outer_width,
                    display_height: tray.outer_height,
                    required_depth: tray.outer_depth,
                },
                Some(tray.front_lip_height),
            )
        } else {
            (
                StockingMode::Loose,
                PlacementGeometry {
                    display_width: Length::from_sixteenths(
                        product.dimensions.width.sixteenths() * placement.facings_x as i32,
                    ),
                    display_height: Length::from_sixteenths(
                        product.dimensions.height.sixteenths() * placement.facings_y as i32,
                    ),
                    required_depth: Length::from_sixteenths(
                        product.dimensions.depth.sixteenths() * placement.facings_z as i32,
                    ),
                },
                None,
            )
        };
        PlacementView {
            id: placement.id.clone(),
            product_id: placement.product_id.clone(),
            shelf_id: placement.shelf_id.clone(),
            x: placement.x,
            stocking_mode,
            facings_x: placement.facings_x,
            facings_y: placement.facings_y,
            facings_z: placement.facings_z,
            stocked_unit_count: placement
                .facings_x
                .saturating_mul(placement.facings_y)
                .saturating_mul(placement.facings_z),
            geometry,
            tray_front_lip_height,
        }
    }

    pub fn placement_view(&self, placement_id: &PlacementId) -> Option<PlacementView> {
        let placement = self.placement(placement_id)?;
        let product = self.product(&placement.product_id)?;
        Some(Self::placement_view_for(placement, product))
    }

    pub fn placement_views(&self) -> Vec<PlacementView> {
        self.placements
            .iter()
            .filter_map(|placement| {
                self.product(&placement.product_id)
                    .map(|product| Self::placement_view_for(placement, product))
            })
            .collect()
    }

    fn display_width(placement: &Placement, product: &Product) -> Length {
        Self::placement_view_for(placement, product)
            .geometry
            .display_width
    }

    fn display_height(placement: &Placement, product: &Product) -> Length {
        Self::placement_view_for(placement, product)
            .geometry
            .display_height
    }

    fn required_depth(placement: &Placement, product: &Product) -> Length {
        Self::placement_view_for(placement, product)
            .geometry
            .required_depth
    }

    fn resolve_add_facings(
        product: &Product,
        shelf_id: &ShelfId,
        facings_x: Option<u32>,
        facings_y: Option<u32>,
        facings_z: Option<u32>,
    ) -> Result<(u32, u32, u32), ValidationSummary> {
        let expected = product
            .tray
            .as_ref()
            .map(|tray| (tray.facings_x, 1, tray.units_deep));
        if let Some((expected_x, expected_y, expected_z)) = expected {
            let resolved = (
                facings_x.unwrap_or(expected_x),
                facings_y.unwrap_or(expected_y),
                facings_z.unwrap_or(expected_z),
            );
            if resolved != (expected_x, expected_y, expected_z) {
                return Err(ValidationSummary {
                    issues: vec![ValidationIssue {
                        code: ValidationCode::TrayFacingMismatch,
                        shelf_id: Some(shelf_id.clone()),
                        message: format!(
                            "{} must use its shelf-ready tray preset of {} × {} × {} facings.",
                            product.description, expected_x, expected_y, expected_z
                        ),
                    }],
                });
            }
            Ok(resolved)
        } else {
            Ok((
                facings_x.unwrap_or(1),
                facings_y.unwrap_or(1),
                facings_z.unwrap_or(1),
            ))
        }
    }

    fn validate_product(product: &Product) -> ValidationSummary {
        let mut validation = ValidationSummary::default();
        if product.dimensions.width <= Length::ZERO
            || product.dimensions.height <= Length::ZERO
            || product.dimensions.depth <= Length::ZERO
            || product.dimensions.source.trim().is_empty()
            || product.dimensions.confidence.trim().is_empty()
        {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::InvalidProductDimensions,
                shelf_id: None,
                message: format!(
                    "Product {} must have positive sourced width, height, and depth.",
                    product.id.0
                ),
            });
        }
        if product.net_weight_ounces_hundredths == 0 {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::InvalidNetWeight,
                shelf_id: None,
                message: format!("Product {} must have a positive net weight.", product.id.0),
            });
        }
        if product.casepack_quantity == 0 {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::InvalidCasepackQuantity,
                shelf_id: None,
                message: format!("Product {} must have a positive casepack.", product.id.0),
            });
        }
        if product.performance.source.trim().is_empty()
            || product.performance.period.trim().is_empty()
            || product.performance.gross_margin_basis_points > 10_000
        {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::InvalidProductPerformance,
                shelf_id: None,
                message: format!(
                    "Product {} must have sourced performance data and gross margin at or below 100%.",
                    product.id.0
                ),
            });
        }
        if let Some(tray) = &product.tray {
            let stocked_units = tray.facings_x.checked_mul(tray.units_deep);
            let required_width =
                i64::from(product.dimensions.width.sixteenths()) * i64::from(tray.facings_x);
            let required_depth =
                i64::from(product.dimensions.depth.sixteenths()) * i64::from(tray.units_deep);
            let valid_capacity = stocked_units.is_some_and(|count| {
                count > 0
                    && product.casepack_quantity > 0
                    && product.casepack_quantity.is_multiple_of(count)
            });
            if tray.facings_x == 0
                || tray.facings_x > 100
                || tray.units_deep == 0
                || tray.units_deep > 100
                || tray.outer_width <= Length::ZERO
                || tray.outer_height <= Length::ZERO
                || tray.outer_depth <= Length::ZERO
                || tray.front_lip_height <= Length::ZERO
                || tray.front_lip_height > tray.outer_height
                || i64::from(tray.outer_width.sixteenths()) < required_width
                || tray.outer_height < product.dimensions.height
                || i64::from(tray.outer_depth.sixteenths()) < required_depth
                || !valid_capacity
            {
                validation.issues.push(ValidationIssue {
                    code: ValidationCode::InvalidTrayConfiguration,
                    shelf_id: None,
                    message: format!(
                        "Product {} has an invalid shelf-ready tray configuration.",
                        product.id.0
                    ),
                });
            }
        }
        validation
    }

    fn shelf_clearance(&self, shelf: &Shelf) -> Length {
        let Some(section) = self
            .fixture
            .sections
            .iter()
            .find(|section| section.id == shelf.section_id)
        else {
            return self.fixture.height - shelf.elevation;
        };
        section
            .shelves
            .iter()
            .filter(|candidate| candidate.elevation > shelf.elevation)
            .map(|candidate| candidate.elevation)
            .min()
            .unwrap_or(section.height)
            - shelf.elevation
    }

    fn validate_placement_location(
        &self,
        placement: &Placement,
        product: &Product,
        target_shelf: &Shelf,
        target_x: Length,
    ) -> ValidationSummary {
        self.validate_placement_location_with_grid(placement, product, target_shelf, target_x, true)
    }

    fn validate_placement_location_with_grid(
        &self,
        placement: &Placement,
        product: &Product,
        target_shelf: &Shelf,
        target_x: Length,
        enforce_x_increment: bool,
    ) -> ValidationSummary {
        let mut validation = ValidationSummary::default();
        let shelf_id = Some(target_shelf.id.clone());
        if target_shelf.kind == ShelfKind::BaseDeck {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::PlacementOnFixedShelf,
                shelf_id,
                message: "The base deck is fixed and cannot accept a placement move.".into(),
            });
            return validation;
        }

        if placement.facings_x == 0
            || placement.facings_y == 0
            || placement.facings_z == 0
            || placement.facings_x > 100
            || placement.facings_y > 100
            || placement.facings_z > 100
        {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::InvalidFacingCount,
                shelf_id: shelf_id.clone(),
                message: "A placement must have between 1 and 100 facings in every dimension."
                    .into(),
            });
        }

        if let Some(tray) = &product.tray {
            if placement.facings_x != tray.facings_x
                || placement.facings_y != 1
                || placement.facings_z != tray.units_deep
            {
                validation.issues.push(ValidationIssue {
                    code: ValidationCode::TrayFacingMismatch,
                    shelf_id: shelf_id.clone(),
                    message: format!(
                        "{} must use its shelf-ready tray preset of {} × 1 × {} facings.",
                        product.description, tray.facings_x, tray.units_deep
                    ),
                });
            }
        }

        if enforce_x_increment && target_x.sixteenths().rem_euclid(2) != 0 {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::PlacementXIncrement,
                shelf_id: shelf_id.clone(),
                message: "Placement positions must use 1/8-inch increments.".into(),
            });
        }

        let width = Self::display_width(placement, product);
        if target_x < Length::ZERO || target_x + width > target_shelf.width {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::PlacementOutOfBounds,
                shelf_id: shelf_id.clone(),
                message:
                    "The placement must remain within the selected shelf's left and right bounds."
                        .into(),
            });
        }

        let target_end = target_x + width;
        for other in &self.placements {
            if other.id == placement.id || other.shelf_id != target_shelf.id {
                continue;
            }
            let Some(other_product) = self
                .products
                .iter()
                .find(|candidate| candidate.id == other.product_id)
            else {
                continue;
            };
            let other_end = other.x + Self::display_width(other, other_product);
            if target_x < other_end && other.x < target_end {
                validation.issues.push(ValidationIssue {
                    code: ValidationCode::PlacementOverlap,
                    shelf_id: shelf_id.clone(),
                    message: "The placement would overlap another product on the selected shelf."
                        .into(),
                });
                break;
            }
            if target_x < other_end + MIN_PLACEMENT_GAP && other.x < target_end + MIN_PLACEMENT_GAP
            {
                validation.issues.push(ValidationIssue {
                    code: ValidationCode::PlacementGap,
                    shelf_id: shelf_id.clone(),
                    message: "Products on the same shelf must keep at least a 1/8-inch gap.".into(),
                });
                break;
            }
        }

        if Self::required_depth(placement, product) > target_shelf.depth {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::PlacementTooDeep,
                shelf_id: shelf_id.clone(),
                message: "The placement requires more depth than the selected shelf provides."
                    .into(),
            });
        }

        if Self::display_height(placement, product) > self.shelf_clearance(target_shelf) {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::PlacementTooTall,
                shelf_id,
                message:
                    "The placement is too tall for the vertical clearance above the selected shelf."
                        .into(),
            });
        }
        validation
    }

    /// Validates the complete current draft without mutating geometry,
    /// revision, history, selection, or renderer state.
    pub fn validate_planogram(&self) -> PlanogramValidationResult {
        let mut validation = ValidationSummary::default();
        let mut adjustable_elevations: Vec<Length> = Vec::new();
        let mut product_ids = Vec::new();
        let mut upcs = Vec::new();

        for product in &self.products {
            validation
                .issues
                .extend(Self::validate_product(product).issues);
            if product_ids.iter().any(|id| id == &product.id) {
                validation.issues.push(ValidationIssue {
                    code: ValidationCode::DuplicateProductId,
                    shelf_id: None,
                    message: format!("Product ID {} appears more than once.", product.id.0),
                });
            } else {
                product_ids.push(product.id.clone());
            }
            if upcs.iter().any(|upc| upc == &product.upc) {
                validation.issues.push(ValidationIssue {
                    code: ValidationCode::DuplicateUpc,
                    shelf_id: None,
                    message: format!("UPC {} appears more than once.", product.upc),
                });
            } else {
                upcs.push(product.upc.clone());
            }
        }

        for shelf in self
            .fixture
            .sections
            .iter()
            .flat_map(|section| &section.shelves)
        {
            match shelf.kind {
                ShelfKind::BaseDeck => {
                    if shelf.elevation != Length::ZERO {
                        validation.issues.push(ValidationIssue {
                            code: ValidationCode::ShelfNotAdjustable,
                            shelf_id: Some(shelf.id.clone()),
                            message: "The base deck must remain fixed at elevation 0.".into(),
                        });
                    }
                }
                ShelfKind::Adjustable => {
                    if shelf.elevation <= Length::ZERO {
                        validation.issues.push(ValidationIssue {
                            code: ValidationCode::BelowBaseDeck,
                            shelf_id: Some(shelf.id.clone()),
                            message: "Adjustable shelves must stay above the base deck.".into(),
                        });
                    }
                    if shelf.elevation > self.fixture.height {
                        validation.issues.push(ValidationIssue {
                            code: ValidationCode::AboveFixture,
                            shelf_id: Some(shelf.id.clone()),
                            message: "Shelf elevation must remain within the fixture.".into(),
                        });
                    }
                    if shelf.elevation.sixteenths().rem_euclid(16) != 0 {
                        validation.issues.push(ValidationIssue {
                            code: ValidationCode::ElevationIncrement,
                            shelf_id: Some(shelf.id.clone()),
                            message: "Shelf elevation must use 1-inch increments.".into(),
                        });
                    }
                    if adjustable_elevations.contains(&shelf.elevation) {
                        validation.issues.push(ValidationIssue {
                            code: ValidationCode::DuplicateElevation,
                            shelf_id: Some(shelf.id.clone()),
                            message: "Two adjustable shelves cannot occupy the same elevation."
                                .into(),
                        });
                    }
                    adjustable_elevations.push(shelf.elevation);
                }
            }
        }

        for placement in &self.placements {
            let product = self
                .products
                .iter()
                .find(|product| product.id == placement.product_id);
            let shelf = self.shelf(&placement.shelf_id);

            if product.is_none() {
                validation.issues.push(ValidationIssue {
                    code: ValidationCode::MissingProduct,
                    shelf_id: Some(placement.shelf_id.clone()),
                    message: format!(
                        "Placement {} references missing product {}.",
                        placement.id.0, placement.product_id.0
                    ),
                });
            }
            if shelf.is_none() {
                validation.issues.push(ValidationIssue {
                    code: ValidationCode::MissingShelf,
                    shelf_id: Some(placement.shelf_id.clone()),
                    message: format!(
                        "Placement {} references missing shelf {}.",
                        placement.id.0, placement.shelf_id.0
                    ),
                });
            }
            if let (Some(product), Some(shelf)) = (product, shelf) {
                validation.issues.extend(
                    self.validate_placement_location(placement, product, shelf, placement.x)
                        .issues,
                );
            }
        }

        PlanogramValidationResult {
            revision: self.revision,
            valid: validation.valid(),
            validation,
        }
    }

    pub fn render_scene(&self) -> RenderScene {
        let mut shelves: Vec<_> = self
            .fixture
            .sections
            .iter()
            .flat_map(|section| &section.shelves)
            .map(|shelf| ShelfSceneNode {
                id: shelf.id.clone(),
                kind: shelf.kind,
                width: shelf.width,
                depth: shelf.depth,
                elevation: shelf.elevation,
            })
            .collect();
        shelves.sort_by(|a, b| match a.elevation.cmp(&b.elevation) {
            Ordering::Equal => a.id.cmp(&b.id),
            ordering => ordering,
        });
        let mut placements = self
            .placements
            .iter()
            .filter_map(|placement| {
                let product = self
                    .products
                    .iter()
                    .find(|product| product.id == placement.product_id)?;
                let view = Self::placement_view_for(placement, product);
                Some(PlacementSceneNode {
                    id: placement.id.clone(),
                    product_id: product.id.clone(),
                    shelf_id: placement.shelf_id.clone(),
                    x: placement.x,
                    width: view.geometry.display_width,
                    height: view.geometry.display_height,
                    required_depth: view.geometry.required_depth,
                    stocking_mode: view.stocking_mode,
                    stocked_unit_count: view.stocked_unit_count,
                    facings_x: view.facings_x,
                    facings_y: view.facings_y,
                    facings_z: view.facings_z,
                    tray_front_lip_height: view.tray_front_lip_height,
                    color: product.color,
                    lid_color: product.lid_color,
                })
            })
            .collect::<Vec<_>>();
        placements.sort_by(|a, b| {
            a.shelf_id
                .cmp(&b.shelf_id)
                .then_with(|| a.x.cmp(&b.x))
                .then_with(|| a.id.cmp(&b.id))
        });
        RenderScene {
            revision: self.revision,
            fixture_id: self.fixture.id.clone(),
            width: self.fixture.width,
            height: self.fixture.height,
            shelves,
            placements,
        }
    }

    #[allow(clippy::result_large_err)]
    fn validate_shelf_elevation_change(
        &self,
        shelf_id: &ShelfId,
        elevation: Length,
    ) -> Result<Length, CommandResult> {
        let Some(shelf) = self.shelf(shelf_id) else {
            return Err(CommandResult::NotFound {
                entity: "shelf".into(),
                id: shelf_id.0.clone(),
            });
        };
        let before = shelf.elevation;
        let mut validation = ValidationSummary::default();
        if shelf.kind != ShelfKind::Adjustable {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::ShelfNotAdjustable,
                shelf_id: Some(shelf_id.clone()),
                message: "The base deck is fixed and cannot move.".into(),
            });
        }
        if elevation <= Length::ZERO {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::BelowBaseDeck,
                shelf_id: Some(shelf_id.clone()),
                message: "Adjustable shelves must stay above the base deck.".into(),
            });
        }
        if elevation > self.fixture.height {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::AboveFixture,
                shelf_id: Some(shelf_id.clone()),
                message: "Shelf elevation must remain within the 7-foot fixture.".into(),
            });
        }
        if elevation.sixteenths().rem_euclid(16) != 0 {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::ElevationIncrement,
                shelf_id: Some(shelf_id.clone()),
                message: "Shelf elevation must use 1-inch increments.".into(),
            });
        }
        if self
            .fixture
            .sections
            .iter()
            .flat_map(|section| &section.shelves)
            .any(|other| {
                other.kind == ShelfKind::Adjustable
                    && other.id != *shelf_id
                    && other.elevation == elevation
            })
        {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::DuplicateElevation,
                shelf_id: Some(shelf_id.clone()),
                message: "Two adjustable shelves cannot occupy the same elevation.".into(),
            });
        }
        for placement in &self.placements {
            let Some(product) = self
                .products
                .iter()
                .find(|product| product.id == placement.product_id)
            else {
                continue;
            };
            let Some(placement_shelf) = self.shelf(&placement.shelf_id) else {
                continue;
            };
            let placement_elevation = if placement.shelf_id == *shelf_id {
                elevation
            } else {
                placement_shelf.elevation
            };
            let next_elevation = self
                .fixture
                .sections
                .iter()
                .flat_map(|section| &section.shelves)
                .map(|candidate| {
                    if candidate.id == *shelf_id {
                        elevation
                    } else {
                        candidate.elevation
                    }
                })
                .filter(|candidate_elevation| *candidate_elevation > placement_elevation)
                .min()
                .unwrap_or(self.fixture.height);
            if placement_elevation + Self::display_height(placement, product) > next_elevation {
                validation.issues.push(ValidationIssue {
                    code: ValidationCode::ProductTooTall,
                    shelf_id: Some(placement.shelf_id.clone()),
                    message: format!(
                        "Moving this shelf would leave too little clearance for {} {}.",
                        product.brand, product.description
                    ),
                });
            }
        }
        if !validation.valid() {
            return Err(CommandResult::ValidationFailed {
                revision: self.revision,
                validation,
            });
        }
        Ok(before)
    }

    pub fn move_shelf(
        &mut self,
        version_id: &VersionId,
        shelf_id: &ShelfId,
        elevation: Length,
        expected_revision: u64,
        reason: impl Into<String>,
    ) -> CommandResult {
        if version_id != &self.id {
            return CommandResult::NotFound {
                entity: "version".into(),
                id: version_id.0.clone(),
            };
        }
        if expected_revision != self.revision {
            return CommandResult::RevisionConflict {
                expected_revision,
                current_revision: self.revision,
            };
        }
        if !self.status.is_editable() {
            return CommandResult::Forbidden {
                message: "This version is not editable.".into(),
            };
        }
        let before = match self.validate_shelf_elevation_change(shelf_id, elevation) {
            Ok(before) => before,
            Err(result) => return result,
        };
        self.apply_move(
            shelf_id,
            before,
            elevation,
            reason.into(),
            "human".into(),
            None,
        )
    }

    pub fn move_placement(
        &mut self,
        version_id: &VersionId,
        placement_id: &PlacementId,
        target_shelf_id: &ShelfId,
        target_x: Length,
        expected_revision: u64,
        reason: impl Into<String>,
    ) -> CommandResult {
        if version_id != &self.id {
            return CommandResult::NotFound {
                entity: "version".into(),
                id: version_id.0.clone(),
            };
        }
        if expected_revision != self.revision {
            return CommandResult::RevisionConflict {
                expected_revision,
                current_revision: self.revision,
            };
        }
        if !self.status.is_editable() {
            return CommandResult::Forbidden {
                message: "This version is not editable.".into(),
            };
        }
        let Some(placement) = self.placement(placement_id).cloned() else {
            return CommandResult::NotFound {
                entity: "placement".into(),
                id: placement_id.0.clone(),
            };
        };
        if self.shelf(&placement.shelf_id).is_none() {
            return CommandResult::NotFound {
                entity: "shelf".into(),
                id: placement.shelf_id.0.clone(),
            };
        }
        let Some(target_shelf) = self.shelf(target_shelf_id).cloned() else {
            return CommandResult::NotFound {
                entity: "shelf".into(),
                id: target_shelf_id.0.clone(),
            };
        };
        let Some(product) = self
            .products
            .iter()
            .find(|product| product.id == placement.product_id)
            .cloned()
        else {
            return CommandResult::NotFound {
                entity: "product".into(),
                id: placement.product_id.0.clone(),
            };
        };
        if placement.shelf_id == *target_shelf_id && placement.x == target_x {
            return CommandResult::InvalidCommand {
                message: "The placement is already at the requested shelf position.".into(),
            };
        }

        let validation =
            self.validate_placement_location(&placement, &product, &target_shelf, target_x);
        if !validation.valid() {
            return CommandResult::ValidationFailed {
                revision: self.revision,
                validation,
            };
        }
        self.apply_move_placement(
            placement_id,
            PlacementLocation {
                shelf_id: placement.shelf_id.clone(),
                x: placement.x,
            },
            PlacementLocation {
                shelf_id: target_shelf_id.clone(),
                x: target_x,
            },
            reason.into(),
            "human".into(),
            None,
        )
    }

    pub fn distribute_shelf(
        &mut self,
        version_id: &VersionId,
        shelf_id: &ShelfId,
        distribution: ShelfDistribution,
        expected_revision: u64,
        reason: impl Into<String>,
    ) -> CommandResult {
        self.distribute_shelf_as(
            version_id,
            shelf_id,
            distribution,
            expected_revision,
            "human",
            reason,
        )
    }

    pub fn distribute_shelf_as(
        &mut self,
        version_id: &VersionId,
        shelf_id: &ShelfId,
        distribution: ShelfDistribution,
        expected_revision: u64,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> CommandResult {
        if version_id != &self.id {
            return CommandResult::NotFound {
                entity: "version".into(),
                id: version_id.0.clone(),
            };
        }
        if expected_revision != self.revision {
            return CommandResult::RevisionConflict {
                expected_revision,
                current_revision: self.revision,
            };
        }
        if !self.status.is_editable() {
            return CommandResult::Forbidden {
                message: "This version is not editable.".into(),
            };
        }
        let Some(shelf) = self.shelf(shelf_id).cloned() else {
            return CommandResult::NotFound {
                entity: "shelf".into(),
                id: shelf_id.0.clone(),
            };
        };
        if shelf.kind == ShelfKind::BaseDeck {
            return CommandResult::ValidationFailed {
                revision: self.revision,
                validation: ValidationSummary {
                    issues: vec![ValidationIssue {
                        code: ValidationCode::PlacementOnFixedShelf,
                        shelf_id: Some(shelf.id),
                        message: "The base deck is fixed and cannot distribute products.".into(),
                    }],
                },
            };
        }

        let mut ordered = self
            .placements
            .iter()
            .filter(|placement| placement.shelf_id == *shelf_id)
            .collect::<Vec<_>>();
        ordered.sort_by(|left, right| left.x.cmp(&right.x).then_with(|| left.id.cmp(&right.id)));
        if ordered.is_empty() {
            return CommandResult::InvalidCommand {
                message: "The selected shelf has no products to distribute.".into(),
            };
        }

        let mut widths = Vec::with_capacity(ordered.len());
        for placement in &ordered {
            let Some(product) = self
                .products
                .iter()
                .find(|product| product.id == placement.product_id)
            else {
                return CommandResult::NotFound {
                    entity: "product".into(),
                    id: placement.product_id.0.clone(),
                };
            };
            widths.push(Self::display_width(placement, product));
        }
        let Some(positions) = resolve_shelf_distribution(&widths, shelf.width, distribution) else {
            return CommandResult::ValidationFailed {
                revision: self.revision,
                validation: ValidationSummary {
                    issues: vec![ValidationIssue {
                        code: ValidationCode::NoShelfCapacity,
                        shelf_id: Some(shelf.id),
                        message:
                            "The shelf cannot fit these products with the required 1/8-inch gaps."
                                .into(),
                    }],
                },
            };
        };
        let changes = ordered
            .into_iter()
            .zip(positions)
            .filter(|(placement, x)| placement.x != *x)
            .map(|(placement, x)| PlacementChange::Move {
                placement_id: placement.id.clone(),
                shelf_id: shelf_id.clone(),
                sequence: 0,
                resolved_x: Some(x),
            })
            .collect::<Vec<_>>();
        if changes.is_empty() {
            return CommandResult::InvalidCommand {
                message: "Products already use the selected shelf distribution.".into(),
            };
        }
        self.apply_placement_changes_with_compensation(
            version_id,
            &changes,
            expected_revision,
            actor.into(),
            reason.into(),
            None,
        )
    }

    pub fn preview_placement_changes(
        &self,
        version_id: &VersionId,
        changes: &[PlacementChange],
        expected_revision: u64,
    ) -> PreviewResult {
        if version_id != &self.id {
            return PreviewResult::NotFound {
                entity: "version".into(),
                id: version_id.0.clone(),
            };
        }
        if expected_revision != self.revision {
            return PreviewResult::RevisionConflict {
                expected_revision,
                current_revision: self.revision,
            };
        }
        if !self.status.is_editable() {
            return PreviewResult::Forbidden {
                message: "This version is not editable.".into(),
            };
        }
        match self.prepare_placement_changes(changes) {
            Ok(prepared) => {
                let preview_scene = Box::new(prepared.candidate.render_scene());
                PreviewResult::Ready {
                    revision: self.revision,
                    operations: prepared.operations,
                    affected_ids: prepared.affected_ids,
                    validation: ValidationSummary::default(),
                    preview_scene,
                }
            }
            Err(CommandResult::ValidationFailed { validation, .. }) => {
                PreviewResult::ValidationFailed {
                    revision: self.revision,
                    validation,
                }
            }
            Err(CommandResult::NotFound { entity, id }) => PreviewResult::NotFound { entity, id },
            Err(CommandResult::Forbidden { message }) => PreviewResult::Forbidden { message },
            Err(CommandResult::InvalidCommand { message }) => {
                PreviewResult::InvalidCommand { message }
            }
            Err(CommandResult::RevisionConflict {
                expected_revision,
                current_revision,
            }) => PreviewResult::RevisionConflict {
                expected_revision,
                current_revision,
            },
            Err(CommandResult::Applied { .. }) => PreviewResult::InvalidCommand {
                message: "The placement proposal could not be previewed.".into(),
            },
        }
    }

    pub fn apply_placement_changes(
        &mut self,
        version_id: &VersionId,
        changes: &[PlacementChange],
        expected_revision: u64,
        reason: impl Into<String>,
    ) -> CommandResult {
        self.apply_placement_changes_as(version_id, changes, expected_revision, "human", reason)
    }

    pub fn apply_placement_changes_as(
        &mut self,
        version_id: &VersionId,
        changes: &[PlacementChange],
        expected_revision: u64,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> CommandResult {
        self.apply_placement_changes_with_compensation(
            version_id,
            changes,
            expected_revision,
            actor.into(),
            reason.into(),
            None,
        )
    }

    fn apply_placement_changes_with_compensation(
        &mut self,
        version_id: &VersionId,
        changes: &[PlacementChange],
        expected_revision: u64,
        actor: String,
        reason: String,
        compensates: Option<ChangeSetId>,
    ) -> CommandResult {
        if version_id != &self.id {
            return CommandResult::NotFound {
                entity: "version".into(),
                id: version_id.0.clone(),
            };
        }
        if expected_revision != self.revision {
            return CommandResult::RevisionConflict {
                expected_revision,
                current_revision: self.revision,
            };
        }
        if !self.status.is_editable() {
            return CommandResult::Forbidden {
                message: "This version is not editable.".into(),
            };
        }
        let prepared = match self.prepare_placement_changes(changes) {
            Ok(prepared) => prepared,
            Err(result) => return result,
        };
        self.apply_prepared_placement_changes(prepared, reason, actor, compensates)
    }

    #[allow(clippy::result_large_err)]
    fn prepare_placement_changes(
        &self,
        changes: &[PlacementChange],
    ) -> Result<PreparedPlacementChanges, CommandResult> {
        if changes.is_empty() {
            return Err(CommandResult::InvalidCommand {
                message: "A placement proposal must include at least one operation.".into(),
            });
        }

        let mut candidate = self.clone();
        let mut requested_operations = Vec::with_capacity(changes.len());
        let mut affected_ids = Vec::with_capacity(changes.len());
        let mut removed_placement_ids = Vec::new();
        let mut touched_shelves = Vec::new();
        let mut shelf_orders = initial_shelf_orders(&candidate);
        let mut seen_placement_ids = Vec::new();
        let exact_positions = changes.iter().all(|change| match change {
            PlacementChange::Add { resolved_x, .. } | PlacementChange::Move { resolved_x, .. } => {
                resolved_x.is_some()
            }
            PlacementChange::Remove { .. } => true,
        });

        for change in changes {
            match change {
                PlacementChange::Add {
                    placement_id,
                    product_id,
                    shelf_id,
                    sequence,
                    resolved_x,
                    facings_x,
                    facings_y,
                    facings_z,
                } => {
                    let Some(product) = candidate
                        .products
                        .iter()
                        .find(|product| &product.id == product_id)
                        .cloned()
                    else {
                        return Err(CommandResult::NotFound {
                            entity: "product".into(),
                            id: product_id.0.clone(),
                        });
                    };
                    let Some(shelf) = candidate.shelf(shelf_id).cloned() else {
                        return Err(CommandResult::NotFound {
                            entity: "shelf".into(),
                            id: shelf_id.0.clone(),
                        });
                    };
                    let (resolved_facings_x, resolved_facings_y, resolved_facings_z) =
                        Self::resolve_add_facings(
                            &product, &shelf.id, *facings_x, *facings_y, *facings_z,
                        )
                        .map_err(|validation| {
                            CommandResult::ValidationFailed {
                                revision: self.revision,
                                validation,
                            }
                        })?;
                    let id = placement_id.clone().unwrap_or_else(|| {
                        let id =
                            PlacementId::new(format!("placement_{:04}", candidate.next_placement));
                        candidate.next_placement += 1;
                        id
                    });
                    if candidate.placement(&id).is_some() {
                        return Err(CommandResult::InvalidCommand {
                            message: format!("Placement {} already exists.", id.0),
                        });
                    }
                    if resolved_facings_x > 100
                        || resolved_facings_y > 100
                        || resolved_facings_z > 100
                    {
                        let mut validation = ValidationSummary::default();
                        validation.issues.push(ValidationIssue {
                            code: ValidationCode::InvalidFacingCount,
                            shelf_id: Some(shelf_id.clone()),
                            message: "A placement cannot exceed 100 facings in any dimension."
                                .into(),
                        });
                        return Err(CommandResult::ValidationFailed {
                            revision: self.revision,
                            validation,
                        });
                    }
                    let placement = Placement {
                        id: id.clone(),
                        product_id: product.id.clone(),
                        shelf_id: shelf.id.clone(),
                        x: resolved_x.unwrap_or(Length::ZERO),
                        facings_x: resolved_facings_x,
                        facings_y: resolved_facings_y,
                        facings_z: resolved_facings_z,
                    };
                    candidate.placements.push(placement.clone());
                    seen_placement_ids.push(id.clone());
                    requested_operations.push(RequestedPlacementOperation::Add(id.clone()));
                    insert_shelf_order(&mut shelf_orders, &shelf.id, id.clone(), *sequence);
                    push_unique(&mut touched_shelves, shelf.id.clone());
                    push_unique(&mut affected_ids, id.0);
                }
                PlacementChange::Move {
                    placement_id,
                    shelf_id,
                    sequence,
                    resolved_x,
                } => {
                    if seen_placement_ids.iter().any(|id| id == placement_id) {
                        return Err(CommandResult::InvalidCommand {
                            message: format!(
                                "Placement {} may only appear once in a proposal.",
                                placement_id.0
                            ),
                        });
                    }
                    let Some(placement) = candidate.placement(placement_id).cloned() else {
                        return Err(CommandResult::NotFound {
                            entity: "placement".into(),
                            id: placement_id.0.clone(),
                        });
                    };
                    let Some(target_shelf) = candidate.shelf(shelf_id).cloned() else {
                        return Err(CommandResult::NotFound {
                            entity: "shelf".into(),
                            id: shelf_id.0.clone(),
                        });
                    };
                    let before = PlacementLocation {
                        shelf_id: placement.shelf_id.clone(),
                        x: placement.x,
                    };
                    let after_x = resolved_x.unwrap_or(Length::ZERO);
                    let target = candidate
                        .placement_mut(placement_id)
                        .expect("validated placement exists");
                    target.shelf_id = shelf_id.clone();
                    target.x = after_x;
                    requested_operations.push(RequestedPlacementOperation::Move {
                        placement_id: placement_id.clone(),
                        before,
                    });
                    seen_placement_ids.push(placement_id.clone());
                    remove_from_shelf_orders(&mut shelf_orders, placement_id);
                    insert_shelf_order(
                        &mut shelf_orders,
                        &target_shelf.id,
                        placement_id.clone(),
                        *sequence,
                    );
                    push_unique(&mut touched_shelves, placement.shelf_id);
                    push_unique(&mut touched_shelves, target_shelf.id.clone());
                    push_unique(&mut affected_ids, placement_id.0.clone());
                }
                PlacementChange::Remove { placement_id } => {
                    if seen_placement_ids.iter().any(|id| id == placement_id) {
                        return Err(CommandResult::InvalidCommand {
                            message: format!(
                                "Placement {} may only appear once in a proposal.",
                                placement_id.0
                            ),
                        });
                    }
                    let Some(index) = candidate
                        .placements
                        .iter()
                        .position(|placement| &placement.id == placement_id)
                    else {
                        return Err(CommandResult::NotFound {
                            entity: "placement".into(),
                            id: placement_id.0.clone(),
                        });
                    };
                    let placement = candidate.placements.remove(index);
                    seen_placement_ids.push(placement_id.clone());
                    remove_from_shelf_orders(&mut shelf_orders, placement_id);
                    push_unique(&mut touched_shelves, placement.shelf_id.clone());
                    requested_operations.push(RequestedPlacementOperation::Remove(placement));
                    push_unique(&mut affected_ids, placement_id.0.clone());
                    if !removed_placement_ids.iter().any(|id| id == placement_id) {
                        removed_placement_ids.push(placement_id.clone());
                    }
                }
            }
        }

        if !exact_positions {
            for shelf_id in &touched_shelves {
                let ordered_ids = shelf_orders
                    .iter()
                    .find(|(candidate_shelf, _)| candidate_shelf == shelf_id)
                    .map(|(_, ids)| ids.clone())
                    .unwrap_or_default();
                let indices: Vec<usize> = ordered_ids
                    .iter()
                    .filter_map(|id| {
                        candidate
                            .placements
                            .iter()
                            .position(|placement| &placement.id == id)
                    })
                    .collect();
                let mut next_x = Length::ZERO;
                for index in indices {
                    let placement = &mut candidate.placements[index];
                    placement.x = next_x;
                    let product = candidate
                        .products
                        .iter()
                        .find(|product| product.id == placement.product_id)
                        .expect("validated product exists");
                    next_x = align_to_eighth(
                        placement.x + Self::display_width(placement, product) + MIN_PLACEMENT_GAP,
                    );
                }
            }
        }

        // Semantic shelf ordering can reflow placements that were not named in
        // the proposal. Mark those implicit moves before validation so the
        // complete resolved proposal—not only the model-authored IDs—is checked.
        // Without this, an auto-moved trailing placement could overflow a shelf
        // and still be previewed and committed as valid.
        for placement in &candidate.placements {
            let Some(before) = self.placement(&placement.id) else {
                continue;
            };
            if before.shelf_id != placement.shelf_id || before.x != placement.x {
                push_unique(&mut affected_ids, placement.id.0.clone());
            }
        }

        let mut validation = ValidationSummary::default();
        for placement in &candidate.placements {
            if !touched_shelves
                .iter()
                .any(|shelf_id| shelf_id == &placement.shelf_id)
            {
                continue;
            }
            let Some(product) = candidate
                .products
                .iter()
                .find(|product| product.id == placement.product_id)
            else {
                return Err(CommandResult::NotFound {
                    entity: "product".into(),
                    id: placement.product_id.0.clone(),
                });
            };
            let Some(shelf) = candidate.shelf(&placement.shelf_id) else {
                return Err(CommandResult::NotFound {
                    entity: "shelf".into(),
                    id: placement.shelf_id.0.clone(),
                });
            };
            let changed = affected_ids.iter().any(|id| id == &placement.id.0);
            validation.issues.extend(
                candidate
                    .validate_placement_location_with_grid(
                        placement,
                        product,
                        shelf,
                        placement.x,
                        changed && !exact_positions,
                    )
                    .issues,
            );
        }
        if !validation.valid() {
            return Err(CommandResult::ValidationFailed {
                revision: self.revision,
                validation,
            });
        }

        let mut operations = Vec::with_capacity(changes.len() + candidate.placements.len());
        let mut requested_move_ids = Vec::new();
        for requested in requested_operations {
            match requested {
                RequestedPlacementOperation::Add(id) => {
                    let placement = candidate
                        .placement(&id)
                        .expect("validated added placement exists")
                        .clone();
                    operations.push(PlanogramOperation::AddPlacement(AddPlacement { placement }));
                }
                RequestedPlacementOperation::Move {
                    placement_id,
                    before,
                } => {
                    let placement = candidate
                        .placement(&placement_id)
                        .expect("validated moved placement exists")
                        .clone();
                    if placement.shelf_id == before.shelf_id && placement.x == before.x {
                        return Err(CommandResult::InvalidCommand {
                            message: format!(
                                "Placement {} is already at the requested shelf position.",
                                placement_id.0
                            ),
                        });
                    }
                    requested_move_ids.push(placement_id.clone());
                    operations.push(PlanogramOperation::MovePlacement(MovePlacement {
                        placement_id,
                        before,
                        after: PlacementLocation {
                            shelf_id: placement.shelf_id,
                            x: placement.x,
                        },
                    }));
                }
                RequestedPlacementOperation::Remove(placement) => {
                    operations.push(PlanogramOperation::RemovePlacement(RemovePlacement {
                        placement,
                    }));
                }
            }
        }

        for placement in &candidate.placements {
            let Some(before) = self.placement(&placement.id) else {
                continue;
            };
            if before.shelf_id == placement.shelf_id && before.x == placement.x {
                continue;
            }
            if requested_move_ids.iter().any(|id| id == &placement.id)
                || changes.iter().any(|change| {
                    matches!(
                        change,
                        PlacementChange::Add {
                            placement_id: Some(id),
                            ..
                        } if id == &placement.id
                    )
                })
            {
                continue;
            }
            operations.push(PlanogramOperation::MovePlacement(MovePlacement {
                placement_id: placement.id.clone(),
                before: PlacementLocation {
                    shelf_id: before.shelf_id.clone(),
                    x: before.x,
                },
                after: PlacementLocation {
                    shelf_id: placement.shelf_id.clone(),
                    x: placement.x,
                },
            }));
            push_unique(&mut affected_ids, placement.id.0.clone());
        }

        Ok(PreparedPlacementChanges {
            candidate,
            operations,
            affected_ids,
            removed_placement_ids,
        })
    }

    fn apply_prepared_placement_changes(
        &mut self,
        prepared: PreparedPlacementChanges,
        reason: String,
        actor: String,
        compensates: Option<ChangeSetId>,
    ) -> CommandResult {
        let PreparedPlacementChanges {
            candidate,
            operations,
            affected_ids,
            removed_placement_ids,
        } = prepared;
        self.placements = candidate.placements;
        self.next_placement = candidate.next_placement;
        let base_revision = self.revision;
        self.revision += 1;
        let change_set = ChangeSet {
            id: ChangeSetId::new(format!("change_{:04}", self.next_change_set)),
            actor,
            reason,
            base_revision,
            resulting_revision: self.revision,
            operations,
            compensates,
        };
        self.next_change_set += 1;
        self.change_sets.push(change_set.clone());
        let scene = self.render_scene();
        let placements = scene
            .placements
            .into_iter()
            .filter(|node| affected_ids.iter().any(|id| id == &node.id.0))
            .collect();
        let validation = ValidationSummary::default();
        CommandResult::Applied {
            revision: self.revision,
            change_set,
            affected_ids,
            validation: validation.clone(),
            scene_patch: Box::new(ScenePatch {
                revision: self.revision,
                shelves: Vec::new(),
                placements,
                removed_placement_ids,
                validation,
            }),
        }
    }

    fn apply_move(
        &mut self,
        shelf_id: &ShelfId,
        before: Length,
        after: Length,
        reason: String,
        actor: String,
        compensates: Option<ChangeSetId>,
    ) -> CommandResult {
        self.shelf_mut(shelf_id)
            .expect("validated shelf exists")
            .elevation = after;
        let base_revision = self.revision;
        self.revision += 1;
        let change_set = ChangeSet {
            id: ChangeSetId::new(format!("change_{:04}", self.next_change_set)),
            actor,
            reason,
            base_revision,
            resulting_revision: self.revision,
            operations: vec![PlanogramOperation::MoveShelf(MoveShelf {
                shelf_id: shelf_id.clone(),
                before,
                after,
            })],
            compensates,
        };
        self.next_change_set += 1;
        self.change_sets.push(change_set.clone());
        let validation = ValidationSummary::default();
        let node = self
            .render_scene()
            .shelves
            .into_iter()
            .find(|node| node.id == *shelf_id)
            .expect("moved shelf rendered");
        CommandResult::Applied {
            revision: self.revision,
            affected_ids: vec![shelf_id.0.clone()],
            scene_patch: Box::new(ScenePatch {
                revision: self.revision,
                shelves: vec![node],
                placements: Vec::new(),
                removed_placement_ids: Vec::new(),
                validation: validation.clone(),
            }),
            validation,
            change_set,
        }
    }

    fn apply_move_placement(
        &mut self,
        placement_id: &PlacementId,
        before: PlacementLocation,
        after: PlacementLocation,
        reason: String,
        actor: String,
        compensates: Option<ChangeSetId>,
    ) -> CommandResult {
        let placement = self
            .placement_mut(placement_id)
            .expect("validated placement exists");
        placement.shelf_id = after.shelf_id.clone();
        placement.x = after.x;
        let base_revision = self.revision;
        self.revision += 1;
        let change_set = ChangeSet {
            id: ChangeSetId::new(format!("change_{:04}", self.next_change_set)),
            actor,
            reason,
            base_revision,
            resulting_revision: self.revision,
            operations: vec![PlanogramOperation::MovePlacement(MovePlacement {
                placement_id: placement_id.clone(),
                before,
                after,
            })],
            compensates,
        };
        self.next_change_set += 1;
        self.change_sets.push(change_set.clone());
        let node = self
            .render_scene()
            .placements
            .into_iter()
            .find(|node| node.id == *placement_id)
            .expect("moved placement rendered");
        let validation = ValidationSummary::default();
        CommandResult::Applied {
            revision: self.revision,
            affected_ids: vec![placement_id.0.clone()],
            scene_patch: Box::new(ScenePatch {
                revision: self.revision,
                shelves: Vec::new(),
                placements: vec![node],
                removed_placement_ids: Vec::new(),
                validation: validation.clone(),
            }),
            validation,
            change_set,
        }
    }

    pub fn undo_change_set(
        &mut self,
        version_id: &VersionId,
        change_set_id: &ChangeSetId,
        expected_revision: u64,
    ) -> CommandResult {
        self.undo_change_set_as(version_id, change_set_id, expected_revision, "human")
    }

    pub fn undo_change_set_as(
        &mut self,
        version_id: &VersionId,
        change_set_id: &ChangeSetId,
        expected_revision: u64,
        actor: impl Into<String>,
    ) -> CommandResult {
        let actor = actor.into();
        if version_id != &self.id {
            return CommandResult::NotFound {
                entity: "version".into(),
                id: version_id.0.clone(),
            };
        }
        if expected_revision != self.revision {
            return CommandResult::RevisionConflict {
                expected_revision,
                current_revision: self.revision,
            };
        }
        if !self.status.is_editable() {
            return CommandResult::Forbidden {
                message: "This version is not editable.".into(),
            };
        }
        let Some(change_set) = self
            .change_sets
            .iter()
            .find(|change| &change.id == change_set_id)
            .cloned()
        else {
            return CommandResult::NotFound {
                entity: "change_set".into(),
                id: change_set_id.0.clone(),
            };
        };
        if self.change_sets.last().map(|change| &change.id) != Some(change_set_id) {
            return CommandResult::InvalidCommand {
                message: "Only the latest change set is eligible for undo.".into(),
            };
        }
        if let Some(inverse) = inverse_placement_changes(&change_set.operations) {
            return self.apply_placement_changes_with_compensation(
                version_id,
                &inverse,
                expected_revision,
                actor,
                format!("Undo {}", change_set.id.0),
                Some(change_set.id),
            );
        }
        match change_set.operations.first() {
            Some(PlanogramOperation::MoveShelf(operation)) => {
                let current = self
                    .shelf(&operation.shelf_id)
                    .map(|shelf| shelf.elevation)
                    .expect("change set references shelf");
                if let Err(result) =
                    self.validate_shelf_elevation_change(&operation.shelf_id, operation.before)
                {
                    return result;
                }
                self.apply_move(
                    &operation.shelf_id,
                    current,
                    operation.before,
                    format!("Undo {}", change_set.id.0),
                    actor.clone(),
                    Some(change_set.id),
                )
            }
            Some(PlanogramOperation::MovePlacement(_))
            | Some(PlanogramOperation::AddPlacement(_))
            | Some(PlanogramOperation::RemovePlacement(_)) => CommandResult::InvalidCommand {
                message: "Change set has no reversible placement operation.".into(),
            },
            None => CommandResult::InvalidCommand {
                message: "Change set has no reversible operation.".into(),
            },
        }
    }

    pub fn latest_change_set_id(&self) -> Option<&ChangeSetId> {
        self.change_sets.last().map(|change| &change.id)
    }

    pub fn add_placement(
        &mut self,
        version_id: &VersionId,
        product_id: &ProductId,
        shelf_id: &ShelfId,
        expected_revision: u64,
        reason: impl Into<String>,
    ) -> CommandResult {
        self.add_placement_as(
            version_id,
            product_id,
            shelf_id,
            expected_revision,
            "human",
            reason,
        )
    }

    pub fn add_placement_as(
        &mut self,
        version_id: &VersionId,
        product_id: &ProductId,
        shelf_id: &ShelfId,
        expected_revision: u64,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> CommandResult {
        let actor = actor.into();
        if version_id != &self.id {
            return CommandResult::NotFound {
                entity: "version".into(),
                id: version_id.0.clone(),
            };
        }
        if expected_revision != self.revision {
            return CommandResult::RevisionConflict {
                expected_revision,
                current_revision: self.revision,
            };
        }
        if !self.status.is_editable() {
            return CommandResult::Forbidden {
                message: "This version is not editable.".into(),
            };
        }
        let Some(product) = self
            .products
            .iter()
            .find(|product| &product.id == product_id)
            .cloned()
        else {
            return CommandResult::NotFound {
                entity: "product".into(),
                id: product_id.0.clone(),
            };
        };
        let Some(shelf) = self.shelf(shelf_id).cloned() else {
            return CommandResult::NotFound {
                entity: "shelf".into(),
                id: shelf_id.0.clone(),
            };
        };
        let (facings_x, facings_y, facings_z) =
            match Self::resolve_add_facings(&product, shelf_id, None, None, None) {
                Ok(facings) => facings,
                Err(validation) => {
                    return CommandResult::ValidationFailed {
                        revision: self.revision,
                        validation,
                    };
                }
            };
        let mut placement = Placement {
            id: PlacementId::new(format!("placement_{:04}", self.next_placement)),
            product_id: product.id.clone(),
            shelf_id: shelf_id.clone(),
            x: Length::ZERO,
            facings_x,
            facings_y,
            facings_z,
        };
        let placement_view = Self::placement_view_for(&placement, &product);
        let mut validation = Self::validate_product(&product);
        if shelf.kind == ShelfKind::BaseDeck {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::PlacementOnFixedShelf,
                shelf_id: Some(shelf_id.clone()),
                message: "The base deck is fixed and cannot accept product placements.".into(),
            });
        }
        if placement_view.geometry.display_width > shelf.width {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::ProductTooWide,
                shelf_id: Some(shelf_id.clone()),
                message: format!("{} is wider than this shelf.", product.description),
            });
        }
        if placement_view.geometry.required_depth > shelf.depth {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::ProductTooDeep,
                shelf_id: Some(shelf_id.clone()),
                message: format!("{} requires more shelf depth.", product.description),
            });
        }
        let clearance = self
            .fixture
            .sections
            .iter()
            .flat_map(|section| &section.shelves)
            .filter(|candidate| candidate.elevation > shelf.elevation)
            .map(|candidate| candidate.elevation - shelf.elevation)
            .min()
            .unwrap_or(self.fixture.height - shelf.elevation);
        if placement_view.geometry.display_height > clearance {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::ProductTooTall,
                shelf_id: Some(shelf_id.clone()),
                message: format!("{} does not fit below the next shelf.", product.description),
            });
        }
        let mut occupied = self
            .placements
            .iter()
            .filter(|placement| placement.shelf_id == *shelf_id)
            .filter_map(|placement| {
                let placed = self
                    .products
                    .iter()
                    .find(|candidate| candidate.id == placement.product_id)?;
                Some((
                    placement.x,
                    placement.x + Self::display_width(placement, placed),
                ))
            })
            .collect::<Vec<_>>();
        occupied.sort_by_key(|(start, _)| *start);
        let mut x = Length::ZERO;
        for (start, end) in occupied {
            if x + placement_view.geometry.display_width + MIN_PLACEMENT_GAP <= start {
                break;
            }
            x = x.max(align_to_eighth(end + MIN_PLACEMENT_GAP));
        }
        if x + placement_view.geometry.display_width > shelf.width {
            validation.issues.push(ValidationIssue {
                code: ValidationCode::NoShelfCapacity,
                shelf_id: Some(shelf_id.clone()),
                message: format!(
                    "No contiguous space remains for {} on this shelf.",
                    product.description
                ),
            });
        }
        if !validation.valid() {
            return CommandResult::ValidationFailed {
                revision: self.revision,
                validation,
            };
        }
        placement.x = x;
        self.next_placement += 1;
        self.apply_add_placement(placement, reason.into(), actor, None)
    }

    pub fn remove_placement(
        &mut self,
        version_id: &VersionId,
        placement_id: &PlacementId,
        expected_revision: u64,
        reason: impl Into<String>,
    ) -> CommandResult {
        self.remove_placement_as(version_id, placement_id, expected_revision, "human", reason)
    }

    pub fn remove_placement_as(
        &mut self,
        version_id: &VersionId,
        placement_id: &PlacementId,
        expected_revision: u64,
        actor: impl Into<String>,
        reason: impl Into<String>,
    ) -> CommandResult {
        let actor = actor.into();
        if version_id != &self.id {
            return CommandResult::NotFound {
                entity: "version".into(),
                id: version_id.0.clone(),
            };
        }
        if expected_revision != self.revision {
            return CommandResult::RevisionConflict {
                expected_revision,
                current_revision: self.revision,
            };
        }
        if !self.status.is_editable() {
            return CommandResult::Forbidden {
                message: "This version is not editable.".into(),
            };
        }
        if !self
            .placements
            .iter()
            .any(|placement| &placement.id == placement_id)
        {
            return CommandResult::NotFound {
                entity: "placement".into(),
                id: placement_id.0.clone(),
            };
        }
        self.apply_remove_placement(placement_id, reason.into(), actor, None)
    }

    fn apply_add_placement(
        &mut self,
        placement: Placement,
        reason: String,
        actor: String,
        compensates: Option<ChangeSetId>,
    ) -> CommandResult {
        self.placements.push(placement.clone());
        let base_revision = self.revision;
        self.revision += 1;
        let change_set = ChangeSet {
            id: ChangeSetId::new(format!("change_{:04}", self.next_change_set)),
            actor,
            reason,
            base_revision,
            resulting_revision: self.revision,
            operations: vec![PlanogramOperation::AddPlacement(AddPlacement {
                placement: placement.clone(),
            })],
            compensates,
        };
        self.next_change_set += 1;
        self.change_sets.push(change_set.clone());
        let node = self
            .render_scene()
            .placements
            .into_iter()
            .find(|node| node.id == placement.id)
            .expect("placed product rendered");
        let validation = ValidationSummary::default();
        CommandResult::Applied {
            revision: self.revision,
            affected_ids: vec![placement.id.0.clone()],
            scene_patch: Box::new(ScenePatch {
                revision: self.revision,
                shelves: Vec::new(),
                placements: vec![node],
                removed_placement_ids: Vec::new(),
                validation: validation.clone(),
            }),
            validation,
            change_set,
        }
    }

    fn apply_remove_placement(
        &mut self,
        placement_id: &PlacementId,
        reason: String,
        actor: String,
        compensates: Option<ChangeSetId>,
    ) -> CommandResult {
        let Some(index) = self
            .placements
            .iter()
            .position(|placement| &placement.id == placement_id)
        else {
            return CommandResult::NotFound {
                entity: "placement".into(),
                id: placement_id.0.clone(),
            };
        };
        let placement = self.placements.remove(index);
        let base_revision = self.revision;
        self.revision += 1;
        let change_set = ChangeSet {
            id: ChangeSetId::new(format!("change_{:04}", self.next_change_set)),
            actor,
            reason,
            base_revision,
            resulting_revision: self.revision,
            operations: vec![PlanogramOperation::RemovePlacement(RemovePlacement {
                placement: placement.clone(),
            })],
            compensates,
        };
        self.next_change_set += 1;
        self.change_sets.push(change_set.clone());
        let validation = ValidationSummary::default();
        CommandResult::Applied {
            revision: self.revision,
            affected_ids: vec![placement.id.0.clone()],
            scene_patch: Box::new(ScenePatch {
                revision: self.revision,
                shelves: Vec::new(),
                placements: Vec::new(),
                removed_placement_ids: vec![placement.id],
                validation: validation.clone(),
            }),
            validation,
            change_set,
        }
    }
}

fn push_unique<T: PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn initial_shelf_orders(version: &DraftVersion) -> Vec<(ShelfId, Vec<PlacementId>)> {
    version
        .fixture
        .sections
        .iter()
        .flat_map(|section| &section.shelves)
        .map(|shelf| {
            let mut placements = version
                .placements
                .iter()
                .filter(|placement| placement.shelf_id == shelf.id)
                .collect::<Vec<_>>();
            placements
                .sort_by(|left, right| left.x.cmp(&right.x).then_with(|| left.id.cmp(&right.id)));
            (
                shelf.id.clone(),
                placements
                    .into_iter()
                    .map(|placement| placement.id.clone())
                    .collect(),
            )
        })
        .collect()
}

fn insert_shelf_order(
    orders: &mut Vec<(ShelfId, Vec<PlacementId>)>,
    shelf_id: &ShelfId,
    placement_id: PlacementId,
    sequence: u32,
) {
    let Some((_, order)) = orders
        .iter_mut()
        .find(|(candidate, _)| candidate == shelf_id)
    else {
        orders.push((shelf_id.clone(), vec![placement_id]));
        return;
    };
    let position = (sequence as usize).min(order.len());
    order.insert(position, placement_id);
}

fn remove_from_shelf_orders(
    orders: &mut [(ShelfId, Vec<PlacementId>)],
    placement_id: &PlacementId,
) {
    for (_, order) in orders {
        if let Some(index) = order.iter().position(|candidate| candidate == placement_id) {
            order.remove(index);
            return;
        }
    }
}

fn align_to_eighth(value: Length) -> Length {
    let units = value.sixteenths();
    Length::from_sixteenths(units + units.rem_euclid(2))
}

fn align_down_to_eighth(value: Length) -> Length {
    let units = value.sixteenths();
    Length::from_sixteenths(units - units.rem_euclid(2))
}

fn distribute_steps(total_steps: i32, slots: usize) -> Vec<i32> {
    debug_assert!(total_steps >= 0);
    debug_assert!(slots > 0);
    let base = total_steps / slots as i32;
    let remainder = total_steps % slots as i32;
    (0..slots)
        .map(|index| base + i32::from((index as i32) < remainder))
        .collect()
}

fn resolve_shelf_distribution(
    widths: &[Length],
    shelf_width: Length,
    distribution: ShelfDistribution,
) -> Option<Vec<Length>> {
    if widths.is_empty() {
        return Some(Vec::new());
    }
    let mut positions = Vec::with_capacity(widths.len());
    let mut next_x = Length::ZERO;
    for width in widths {
        positions.push(next_x);
        next_x = align_to_eighth(next_x + *width + MIN_PLACEMENT_GAP);
    }
    let packed_end = positions.last().copied()? + *widths.last()?;
    if packed_end > shelf_width {
        return None;
    }
    let slack = shelf_width - packed_end;
    match distribution {
        ShelfDistribution::PackedLeft => {}
        ShelfDistribution::Centered => {
            let offset = align_down_to_eighth(Length::from_sixteenths(slack.sixteenths() / 2));
            for position in &mut positions {
                *position = *position + offset;
            }
        }
        ShelfDistribution::SpaceBetween if widths.len() == 1 => {
            let offset = align_down_to_eighth(Length::from_sixteenths(slack.sixteenths() / 2));
            positions[0] = offset;
        }
        ShelfDistribution::SpaceBetween => {
            let bonuses = distribute_steps(slack.sixteenths() / 2, widths.len() - 1);
            let mut accumulated = 0;
            for (index, position) in positions.iter_mut().enumerate().skip(1) {
                accumulated += bonuses[index - 1] * 2;
                *position = *position + Length::from_sixteenths(accumulated);
            }
        }
        ShelfDistribution::SpaceEvenly => {
            let bonuses = distribute_steps(slack.sixteenths() / 2, widths.len() + 1);
            let mut accumulated = bonuses[0] * 2;
            positions[0] = positions[0] + Length::from_sixteenths(accumulated);
            for (index, position) in positions.iter_mut().enumerate().skip(1) {
                accumulated += bonuses[index] * 2;
                *position = *position + Length::from_sixteenths(accumulated);
            }
        }
    }
    Some(positions)
}

fn inverse_placement_changes(operations: &[PlanogramOperation]) -> Option<Vec<PlacementChange>> {
    if operations.is_empty()
        || operations
            .iter()
            .any(|operation| matches!(operation, PlanogramOperation::MoveShelf(_)))
    {
        return None;
    }
    Some(
        operations
            .iter()
            .rev()
            .map(|operation| match operation {
                PlanogramOperation::MovePlacement(operation) => PlacementChange::Move {
                    placement_id: operation.placement_id.clone(),
                    shelf_id: operation.before.shelf_id.clone(),
                    sequence: 0,
                    resolved_x: Some(operation.before.x),
                },
                PlanogramOperation::AddPlacement(operation) => PlacementChange::Remove {
                    placement_id: operation.placement.id.clone(),
                },
                PlanogramOperation::RemovePlacement(operation) => PlacementChange::Add {
                    placement_id: Some(operation.placement.id.clone()),
                    product_id: operation.placement.product_id.clone(),
                    shelf_id: operation.placement.shelf_id.clone(),
                    sequence: 0,
                    resolved_x: Some(operation.placement.x),
                    facings_x: Some(operation.placement.facings_x),
                    facings_y: Some(operation.placement.facings_y),
                    facings_z: Some(operation.placement.facings_z),
                },
                PlanogramOperation::MoveShelf(_) => unreachable!("filtered above"),
            })
            .collect(),
    )
}

const PERFORMANCE_SOURCE: &str = "Synthetic representative 13-week average; not retailer actuals";
const PERFORMANCE_PERIOD: &str = "Trailing 13 weeks";
const TRAY_FRONT_LIP_HEIGHT: Length = Length::from_sixteenths(20);

struct CatalogMetrics {
    net_weight_ounces_hundredths: u32,
    sales_per_store_per_week_cents: u64,
    units_per_store_per_week_milliunits: u64,
    gross_margin_basis_points: i32,
    casepack_quantity: u32,
    tray: Option<TrayConfiguration>,
}

fn tray(
    facings_x: u32,
    units_deep: u32,
    outer_width: i32,
    outer_height: i32,
    outer_depth: i32,
) -> Option<TrayConfiguration> {
    Some(TrayConfiguration {
        facings_x,
        units_deep,
        outer_width: Length::from_sixteenths(outer_width),
        outer_height: Length::from_sixteenths(outer_height),
        outer_depth: Length::from_sixteenths(outer_depth),
        front_lip_height: TRAY_FRONT_LIP_HEIGHT,
    })
}

fn catalog_metrics(id: &str) -> CatalogMetrics {
    let (weight, sales, units, margin, casepack, tray) = match id {
        "jif_creamy_16" => (1_600, 3_665, 10_500, 2_850, 12, tray(3, 4, 175, 80, 232)),
        "jif_crunchy_16" => (1_600, 2_024, 5_800, 2_875, 12, None),
        "jif_natural_16" => (1_600, 1_716, 4_300, 3_100, 12, None),
        "jif_creamy_40" => (4_000, 3_895, 5_200, 2_550, 6, None),
        "jif_crunchy_40" => (4_000, 1_947, 2_600, 2_575, 6, None),
        "jif_natural_40" => (4_000, 1_678, 2_100, 2_800, 6, None),
        "skippy_creamy_16" => (1_630, 2_928, 8_900, 2_900, 12, tray(3, 4, 181, 78, 240)),
        "skippy_chunk_16" => (1_630, 1_546, 4_700, 2_925, 12, None),
        "skippy_natural_16" => (1_500, 1_364, 3_600, 3_150, 12, None),
        "skippy_creamy_40" => (4_000, 3_076, 4_400, 2_600, 6, None),
        "skippy_chunk_40" => (4_000, 1_538, 2_200, 2_625, 6, None),
        "skippy_natural_40" => (4_000, 1_348, 1_800, 2_850, 6, None),
        "peter_pan_creamy_16" => (1_630, 1_914, 6_400, 2_750, 12, tray(3, 4, 178, 78, 236)),
        "peter_pan_crunchy_16" => (1_630, 927, 3_100, 2_775, 12, None),
        "peter_pan_creamy_40" => (4_000, 1_947, 3_000, 2_500, 6, None),
        "peter_pan_crunchy_40" => (4_000, 909, 1_400, 2_525, 6, None),
        "smuckers_natural_16" => (1_600, 1_572, 3_500, 3_300, 12, tray(2, 3, 116, 84, 172)),
        "smuckers_chunky_16" => (1_600, 808, 1_800, 3_325, 12, None),
        "smuckers_natural_26" => (2_600, 1_298, 2_000, 3_100, 6, None),
        "smuckers_chunky_26" => (2_600, 649, 1_000, 3_125, 6, None),
        "justins_classic_16" => (1_600, 1_118, 1_600, 3_600, 6, tray(2, 3, 116, 86, 172)),
        "justins_classic_28" => (2_800, 879, 800, 3_400, 6, None),
        _ => panic!("missing catalog metrics for {id}"),
    };
    CatalogMetrics {
        net_weight_ounces_hundredths: weight,
        sales_per_store_per_week_cents: sales,
        units_per_store_per_week_milliunits: units,
        gross_margin_basis_points: margin,
        casepack_quantity: casepack,
        tray,
    }
}

#[allow(clippy::too_many_arguments)]
fn product(
    id: &str,
    upc: &str,
    brand: &str,
    description: &str,
    size_oz: &str,
    width: i32,
    height: i32,
    depth: i32,
    color: [u8; 3],
    lid_color: [u8; 3],
) -> Product {
    let metrics = catalog_metrics(id);
    Product {
        id: ProductId::new(id),
        upc: upc.into(),
        brand: brand.into(),
        description: description.into(),
        size_oz: size_oz.into(),
        category: "Peanut Butter".into(),
        dimensions: ProductDimensions {
            width: Length::from_sixteenths(width),
            height: Length::from_sixteenths(height),
            depth: Length::from_sixteenths(depth),
            source: "Representative package measurement".into(),
            confidence: "medium".into(),
        },
        net_weight_ounces_hundredths: metrics.net_weight_ounces_hundredths,
        casepack_quantity: metrics.casepack_quantity,
        performance: ProductPerformance {
            sales_per_store_per_week_cents: metrics.sales_per_store_per_week_cents,
            units_per_store_per_week_milliunits: metrics.units_per_store_per_week_milliunits,
            gross_margin_basis_points: metrics.gross_margin_basis_points,
            source: PERFORMANCE_SOURCE.into(),
            period: PERFORMANCE_PERIOD.into(),
        },
        tray: metrics.tray,
        color,
        lid_color,
    }
}

#[allow(clippy::too_many_arguments)]
fn tall_product(
    id: &str,
    upc: &str,
    brand: &str,
    description: &str,
    size_oz: &str,
    width: i32,
    height: i32,
    depth: i32,
    color: [u8; 3],
    lid_color: [u8; 3],
) -> Product {
    let mut product = product(
        id,
        upc,
        brand,
        description,
        size_oz,
        width,
        height,
        depth,
        color,
        lid_color,
    );
    product.dimensions.source = "Representative tall family-size package".into();
    product.dimensions.confidence = "concept".into();
    product
}

const JIF_BRAND_COLOR: [u8; 3] = [207, 31, 38];
const SKIPPY_BRAND_COLOR: [u8; 3] = [24, 132, 164];
const PETER_PAN_BRAND_COLOR: [u8; 3] = [37, 127, 75];
const SMUCKERS_BRAND_COLOR: [u8; 3] = [119, 75, 44];
const JUSTINS_BRAND_COLOR: [u8; 3] = [195, 142, 48];

pub fn default_products() -> Vec<Product> {
    vec![
        product(
            "jif_creamy_16",
            "051500255162",
            "Jif",
            "Creamy Peanut Butter",
            "16 oz",
            57,
            78,
            57,
            JIF_BRAND_COLOR,
            [207, 31, 38],
        ),
        product(
            "jif_crunchy_16",
            "051500255179",
            "Jif",
            "Extra Crunchy Peanut Butter",
            "16 oz",
            57,
            78,
            57,
            JIF_BRAND_COLOR,
            [207, 31, 38],
        ),
        product(
            "jif_natural_16",
            "051500243626",
            "Jif",
            "Natural Creamy Peanut Butter Spread",
            "16 oz",
            58,
            79,
            58,
            JIF_BRAND_COLOR,
            [141, 91, 38],
        ),
        tall_product(
            "jif_creamy_40",
            "051500720004",
            "Jif",
            "Creamy Peanut Butter",
            "40 oz",
            68,
            126,
            68,
            JIF_BRAND_COLOR,
            [207, 31, 38],
        ),
        tall_product(
            "jif_crunchy_40",
            "051500720028",
            "Jif",
            "Extra Crunchy Peanut Butter",
            "40 oz family size",
            68,
            126,
            68,
            JIF_BRAND_COLOR,
            [207, 31, 38],
        ),
        tall_product(
            "jif_natural_40",
            "051500243213",
            "Jif",
            "Natural Creamy Peanut Butter Spread",
            "40 oz family size",
            68,
            126,
            68,
            JIF_BRAND_COLOR,
            [141, 91, 38],
        ),
        product(
            "skippy_creamy_16",
            "037600110754",
            "SKIPPY",
            "Creamy Peanut Butter",
            "16.3 oz",
            59,
            76,
            59,
            SKIPPY_BRAND_COLOR,
            [26, 127, 158],
        ),
        product(
            "skippy_chunk_16",
            "037600110761",
            "SKIPPY",
            "SUPER CHUNK Peanut Butter",
            "16.3 oz",
            59,
            76,
            59,
            SKIPPY_BRAND_COLOR,
            [25, 81, 151],
        ),
        product(
            "skippy_natural_16",
            "037600105439",
            "SKIPPY",
            "Natural Creamy Peanut Butter Spread",
            "15 oz",
            58,
            76,
            58,
            SKIPPY_BRAND_COLOR,
            [170, 116, 50],
        ),
        tall_product(
            "skippy_creamy_40",
            "037600106254",
            "SKIPPY",
            "Creamy Peanut Butter",
            "40 oz family size",
            67,
            124,
            67,
            SKIPPY_BRAND_COLOR,
            [26, 127, 158],
        ),
        tall_product(
            "skippy_chunk_40",
            "037600106186",
            "SKIPPY",
            "SUPER CHUNK Peanut Butter",
            "40 oz family size",
            67,
            124,
            67,
            SKIPPY_BRAND_COLOR,
            [25, 81, 151],
        ),
        tall_product(
            "skippy_natural_40",
            "037600106742",
            "SKIPPY",
            "Natural Creamy Peanut Butter Spread",
            "40 oz family size",
            67,
            124,
            67,
            SKIPPY_BRAND_COLOR,
            [170, 116, 50],
        ),
        product(
            "peter_pan_creamy_16",
            "045300005267",
            "Peter Pan",
            "Creamy Peanut Butter",
            "16.3 oz",
            58,
            76,
            58,
            PETER_PAN_BRAND_COLOR,
            [244, 205, 36],
        ),
        product(
            "peter_pan_crunchy_16",
            "045300005250",
            "Peter Pan",
            "Crunchy Peanut Butter",
            "16.3 oz",
            58,
            76,
            58,
            PETER_PAN_BRAND_COLOR,
            [244, 205, 36],
        ),
        tall_product(
            "peter_pan_creamy_40",
            "045300299323",
            "Peter Pan",
            "Creamy Peanut Butter",
            "40 oz family size",
            70,
            126,
            70,
            PETER_PAN_BRAND_COLOR,
            [244, 205, 36],
        ),
        tall_product(
            "peter_pan_crunchy_40",
            "045300299309",
            "Peter Pan",
            "Crunchy Peanut Butter",
            "40 oz family size",
            70,
            126,
            70,
            PETER_PAN_BRAND_COLOR,
            [244, 205, 36],
        ),
        product(
            "smuckers_natural_16",
            "051500017012",
            "Smucker's",
            "Natural Creamy Peanut Butter",
            "16 oz",
            56,
            82,
            56,
            SMUCKERS_BRAND_COLOR,
            [111, 67, 34],
        ),
        product(
            "smuckers_chunky_16",
            "051500017029",
            "Smucker's",
            "Natural Chunky Peanut Butter",
            "16 oz",
            56,
            82,
            56,
            SMUCKERS_BRAND_COLOR,
            [111, 67, 34],
        ),
        tall_product(
            "smuckers_natural_26",
            "051500017036",
            "Smucker's",
            "Natural Creamy Peanut Butter",
            "26 oz tall glass",
            60,
            122,
            60,
            SMUCKERS_BRAND_COLOR,
            [111, 67, 34],
        ),
        tall_product(
            "smuckers_chunky_26",
            "051500017043",
            "Smucker's",
            "Natural Chunky Peanut Butter",
            "26 oz tall glass",
            60,
            122,
            60,
            SMUCKERS_BRAND_COLOR,
            [111, 67, 34],
        ),
        product(
            "justins_classic_16",
            "894455000018",
            "Justin's",
            "Classic Peanut Butter",
            "16 oz",
            56,
            84,
            56,
            JUSTINS_BRAND_COLOR,
            [164, 116, 41],
        ),
        tall_product(
            "justins_classic_28",
            "840379101393",
            "Justin's",
            "Classic Peanut Butter",
            "28 oz family size",
            64,
            124,
            64,
            JUSTINS_BRAND_COLOR,
            [164, 116, 41],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shelf(id: &str) -> ShelfId {
        ShelfId::new(id)
    }

    #[test]
    fn default_fixture_is_exact_and_deterministic() {
        let fixture = default_fixture();
        assert_eq!(fixture.width.sixteenths(), 768);
        assert_eq!(fixture.height.sixteenths(), 1_344);
        let shelves = &fixture.sections[0].shelves;
        assert_eq!(
            shelves
                .iter()
                .filter(|s| s.kind == ShelfKind::BaseDeck)
                .count(),
            1
        );
        assert_eq!(
            shelves
                .iter()
                .filter(|s| s.kind == ShelfKind::Adjustable)
                .count(),
            6
        );
        assert_eq!(
            shelves
                .iter()
                .filter(|s| s.kind == ShelfKind::Adjustable)
                .map(|s| s.elevation.sixteenths())
                .collect::<Vec<_>>(),
            vec![192, 384, 576, 768, 960, 1_152]
        );
    }

    #[test]
    fn exact_length_conversions() {
        assert_eq!(Length::inches(1).sixteenths(), 16);
        assert_eq!(Length::inches(12).sixteenths(), 192);
        assert_eq!(Length::feet(1).sixteenths(), 192);
        assert_eq!(Length::from_sixteenths(200).sixteenths(), 200);
    }

    #[test]
    fn whole_plan_validation_reports_the_current_revision_without_mutation() {
        let draft = DraftVersion::default();
        let before = draft.clone();

        let result = draft.validate_planogram();

        assert_eq!(result.revision, 0);
        assert!(result.valid);
        assert!(result.validation.issues.is_empty());
        assert_eq!(draft, before);
    }

    #[test]
    fn whole_plan_validation_collects_structured_issues_across_the_draft() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        for expected_revision in 0..2 {
            let result = draft.add_placement(
                &version,
                &ProductId::new("jif_creamy_16"),
                &shelf("shelf_01"),
                expected_revision,
                "validation fixture",
            );
            assert!(matches!(result, CommandResult::Applied { .. }));
        }

        draft.placements[0].product_id = ProductId::new("missing_product");
        draft.placements[0].shelf_id = ShelfId::new("missing_shelf");
        draft.placements[1].x = Length::from_sixteenths(1);
        let shelf_01_elevation = draft.shelf(&shelf("shelf_01")).unwrap().elevation;
        draft.shelf_mut(&shelf("shelf_02")).unwrap().elevation = shelf_01_elevation;
        let before = draft.clone();

        let result = draft.validate_planogram();
        let codes = result
            .validation
            .issues
            .iter()
            .map(|issue| issue.code)
            .collect::<Vec<_>>();

        assert_eq!(result.revision, 2);
        assert!(!result.valid);
        assert!(codes.contains(&ValidationCode::MissingProduct));
        assert!(codes.contains(&ValidationCode::MissingShelf));
        assert!(codes.contains(&ValidationCode::PlacementXIncrement));
        assert!(codes.contains(&ValidationCode::DuplicateElevation));
        assert_eq!(draft, before);
    }

    #[test]
    fn valid_move_increments_once_and_returns_patch() {
        let mut draft = DraftVersion::default();
        let result = draft.move_shelf(
            &draft.id.clone(),
            &shelf("shelf_02"),
            Length::from_sixteenths(400),
            0,
            "Inspector edit",
        );
        assert!(matches!(result, CommandResult::Applied { revision: 1, .. }));
        assert_eq!(draft.revision, 1);
        assert_eq!(draft.change_sets.len(), 1);
        assert_eq!(
            draft
                .shelf(&shelf("shelf_02"))
                .unwrap()
                .elevation
                .sixteenths(),
            400
        );
    }

    #[test]
    fn invalid_moves_are_atomic() {
        for (id, elevation) in [
            ("base_deck", 4),
            ("shelf_02", 192),
            ("shelf_02", 1_345),
            ("shelf_02", 0),
            ("shelf_02", 401),
        ] {
            let mut draft = DraftVersion::default();
            let result = draft.move_shelf(
                &draft.id.clone(),
                &shelf(id),
                Length::from_sixteenths(elevation),
                0,
                "Invalid",
            );
            assert!(matches!(result, CommandResult::ValidationFailed { .. }));
            assert_eq!(draft, DraftVersion::default());
        }
    }

    #[test]
    fn shelf_move_validates_derived_vertical_facings_atomically() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        let added = draft.apply_placement_changes_as(
            &version,
            &[PlacementChange::Add {
                placement_id: None,
                product_id: ProductId::new("jif_crunchy_16"),
                shelf_id: shelf("shelf_01"),
                sequence: 0,
                resolved_x: None,
                facings_x: Some(1),
                facings_y: Some(2),
                facings_z: Some(1),
            }],
            0,
            "webmcp",
            "Stack the display two high",
        );
        assert!(matches!(added, CommandResult::Applied { revision: 1, .. }));
        let before = draft.clone();

        let result = draft.move_shelf(
            &version,
            &shelf("shelf_02"),
            Length::from_sixteenths(320),
            1,
            "Reduce clearance",
        );
        assert!(matches!(
            result,
            CommandResult::ValidationFailed { revision: 1, ref validation }
                if validation.issues.iter().any(|issue| issue.code == ValidationCode::ProductTooTall)
        ));
        assert_eq!(draft, before);
    }

    #[test]
    fn stale_revision_is_atomic() {
        let mut draft = DraftVersion::default();
        let result = draft.move_shelf(
            &draft.id.clone(),
            &shelf("shelf_02"),
            Length::from_sixteenths(400),
            9,
            "Stale",
        );
        assert!(matches!(
            result,
            CommandResult::RevisionConflict {
                current_revision: 0,
                ..
            }
        ));
        assert_eq!(draft, DraftVersion::default());
    }

    #[test]
    fn undo_is_compensating_history() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        draft.move_shelf(
            &version,
            &shelf("shelf_02"),
            Length::from_sixteenths(400),
            0,
            "Keyboard move",
        );
        let original = draft.latest_change_set_id().unwrap().clone();
        let result = draft.undo_change_set(&version, &original, 1);
        assert!(matches!(result, CommandResult::Applied { revision: 2, .. }));
        assert_eq!(
            draft
                .shelf(&shelf("shelf_02"))
                .unwrap()
                .elevation
                .sixteenths(),
            384
        );
        assert_eq!(draft.change_sets.len(), 2);
        assert_eq!(draft.change_sets[1].compensates, Some(original));
    }

    #[test]
    fn undo_rejects_non_latest_dependent_changes_atomically() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        let first = draft.move_shelf(
            &version,
            &shelf("shelf_02"),
            Length::from_sixteenths(400),
            0,
            "Move shelf 02",
        );
        let first_id = match first {
            CommandResult::Applied { change_set, .. } => change_set.id,
            result => panic!("unexpected result: {result:?}"),
        };
        let second = draft.move_shelf(
            &version,
            &shelf("shelf_03"),
            Length::from_sixteenths(384),
            1,
            "Move shelf 03 into the old opening",
        );
        let second_id = match second {
            CommandResult::Applied { change_set, .. } => change_set.id,
            result => panic!("unexpected result: {result:?}"),
        };
        let before = draft.clone();

        let rejected = draft.undo_change_set(&version, &first_id, 2);
        assert!(matches!(
            rejected,
            CommandResult::InvalidCommand { ref message }
                if message == "Only the latest change set is eligible for undo."
        ));
        assert_eq!(draft, before);

        let latest = draft.undo_change_set(&version, &second_id, 2);
        assert!(matches!(latest, CommandResult::Applied { revision: 3, .. }));
        assert_eq!(
            draft.shelf(&shelf("shelf_02")).unwrap().elevation,
            Length::from_sixteenths(400)
        );
        assert_eq!(
            draft.shelf(&shelf("shelf_03")).unwrap().elevation,
            Length::from_sixteenths(576)
        );
    }

    #[test]
    fn product_placement_is_first_fit_revisioned_and_only_latest_is_undoable() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        let first = draft.add_placement(
            &version,
            &ProductId::new("jif_creamy_16"),
            &shelf("shelf_01"),
            0,
            "catalog add",
        );
        let first_change = match first {
            CommandResult::Applied {
                revision,
                change_set,
                ref scene_patch,
                ..
            } => {
                assert_eq!(revision, 1);
                assert_eq!(scene_patch.placements[0].x, Length::ZERO);
                change_set.id
            }
            result => panic!("unexpected result: {result:?}"),
        };
        let second = draft.add_placement(
            &version,
            &ProductId::new("skippy_creamy_16"),
            &shelf("shelf_01"),
            1,
            "catalog add",
        );
        let second_change = match second {
            CommandResult::Applied {
                revision: 2,
                change_set,
                ..
            } => change_set.id,
            result => panic!("unexpected result: {result:?}"),
        };
        assert_eq!(draft.placements[1].x, Length::from_sixteenths(178));

        let before_non_latest_undo = draft.clone();
        let rejected = draft.undo_change_set(&version, &first_change, 2);
        assert!(matches!(rejected, CommandResult::InvalidCommand { .. }));
        assert_eq!(draft, before_non_latest_undo);

        let undo = draft.undo_change_set(&version, &second_change, 2);
        assert!(matches!(undo, CommandResult::Applied { revision: 3, .. }));
        assert_eq!(draft.placements.len(), 1);
        assert_eq!(
            draft.placements[0].product_id,
            ProductId::new("jif_creamy_16")
        );

        let blocked_move = draft.move_shelf(
            &version,
            &shelf("shelf_02"),
            Length::inches(15),
            3,
            "reduce clearance",
        );
        assert!(matches!(
            blocked_move,
            CommandResult::ValidationFailed { revision: 3, .. }
        ));
        assert_eq!(
            draft.shelf(&shelf("shelf_02")).unwrap().elevation,
            Length::inches(24)
        );
    }

    #[test]
    fn direct_add_rejects_the_fixed_base_deck_for_human_and_webmcp() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        let before = draft.clone();

        for result in [
            draft.add_placement(
                &version,
                &ProductId::new("jif_creamy_16"),
                &shelf("base_deck"),
                0,
                "catalog add",
            ),
            draft.add_placement_as(
                &version,
                &ProductId::new("jif_creamy_16"),
                &shelf("base_deck"),
                0,
                "webmcp",
                "site-tool add",
            ),
        ] {
            assert!(matches!(
                result,
                CommandResult::ValidationFailed { revision: 0, ref validation }
                    if validation.issues.iter().any(|issue| issue.code == ValidationCode::PlacementOnFixedShelf)
            ));
            assert_eq!(draft, before);
        }
    }

    #[test]
    fn webmcp_add_and_undo_are_explicitly_attributed() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        let added = draft.add_placement_as(
            &version,
            &ProductId::new("jif_creamy_16"),
            &shelf("shelf_01"),
            0,
            "webmcp",
            "Add Jif from the site tool",
        );
        let change_set_id = match added {
            CommandResult::Applied { change_set, .. } => {
                assert_eq!(change_set.actor, "webmcp");
                assert_eq!(change_set.reason, "Add Jif from the site tool");
                change_set.id
            }
            result => panic!("unexpected result: {result:?}"),
        };

        let undone = draft.undo_change_set_as(&version, &change_set_id, 1, "webmcp");
        match undone {
            CommandResult::Applied { change_set, .. } => {
                assert_eq!(change_set.actor, "webmcp");
                assert_eq!(change_set.compensates, Some(change_set_id));
            }
            result => panic!("unexpected result: {result:?}"),
        }
    }

    #[test]
    fn generic_placement_proposal_previews_applies_atomically_and_undoes_as_one_batch() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        let changes = vec![
            PlacementChange::Add {
                placement_id: None,
                product_id: ProductId::new("jif_creamy_16"),
                shelf_id: shelf("shelf_01"),
                sequence: 0,
                resolved_x: None,
                facings_x: None,
                facings_y: None,
                facings_z: None,
            },
            PlacementChange::Add {
                placement_id: None,
                product_id: ProductId::new("jif_creamy_40"),
                shelf_id: shelf("shelf_02"),
                sequence: 0,
                resolved_x: None,
                facings_x: Some(1),
                facings_y: Some(1),
                facings_z: Some(1),
            },
        ];

        let preview = draft.preview_placement_changes(&version, &changes, 0);
        assert!(
            matches!(preview, PreviewResult::Ready { revision: 0, ref operations, .. } if operations.len() == 2)
        );
        assert!(draft.placements.is_empty());
        assert_eq!(draft.revision, 0);
        assert!(draft.change_sets.is_empty());

        let applied = draft.apply_placement_changes_as(
            &version,
            &changes,
            0,
            "webmcp",
            "Group brands, then place smaller packages higher",
        );
        let change_set_id = match applied {
            CommandResult::Applied {
                revision,
                change_set,
                ref affected_ids,
                ref scene_patch,
                ..
            } => {
                assert_eq!(revision, 1);
                assert_eq!(
                    affected_ids,
                    &vec![
                        String::from("placement_0001"),
                        String::from("placement_0002")
                    ]
                );
                assert_eq!(scene_patch.placements.len(), 2);
                assert_eq!(change_set.actor, "webmcp");
                assert_eq!(change_set.operations.len(), 2);
                change_set.id
            }
            result => panic!("unexpected result: {result:?}"),
        };
        assert_eq!(draft.placements.len(), 2);
        assert_eq!(draft.revision, 1);

        let stale_preview = draft.preview_placement_changes(&version, &changes, 0);
        assert!(matches!(
            stale_preview,
            PreviewResult::RevisionConflict {
                current_revision: 1,
                ..
            }
        ));

        let undone = draft.undo_change_set_as(&version, &change_set_id, 1, "webmcp");
        match undone {
            CommandResult::Applied {
                revision,
                change_set,
                ..
            } => {
                assert_eq!(revision, 2);
                assert_eq!(change_set.actor, "webmcp");
                assert_eq!(change_set.compensates, Some(change_set_id));
                assert_eq!(change_set.operations.len(), 2);
            }
            result => panic!("unexpected result: {result:?}"),
        }
        assert!(draft.placements.is_empty());
    }

    #[test]
    fn invalid_generic_placement_proposal_is_atomic() {
        let draft = DraftVersion::default();
        let version = draft.id.clone();
        let before = draft.clone();
        let preview = draft.preview_placement_changes(
            &version,
            &[
                PlacementChange::Add {
                    placement_id: None,
                    product_id: ProductId::new("jif_crunchy_16"),
                    shelf_id: shelf("shelf_01"),
                    sequence: 0,
                    resolved_x: None,
                    facings_x: Some(1),
                    facings_y: Some(1),
                    facings_z: Some(1),
                },
                PlacementChange::Add {
                    placement_id: None,
                    product_id: ProductId::new("skippy_chunk_16"),
                    shelf_id: shelf("shelf_01"),
                    sequence: 1,
                    resolved_x: None,
                    facings_x: Some(100),
                    facings_y: Some(1),
                    facings_z: Some(1),
                },
            ],
            0,
        );
        assert!(matches!(
            preview,
            PreviewResult::ValidationFailed { ref validation, revision: 0 }
                if validation.issues.iter().any(|issue| issue.code == ValidationCode::PlacementOutOfBounds)
        ));
        assert_eq!(draft, before);
    }

    #[test]
    fn generic_proposal_validates_implicit_reflow_before_preview_or_apply() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        for revision in 0..12 {
            assert!(matches!(
                draft.add_placement(
                    &version,
                    &ProductId::new("jif_crunchy_16"),
                    &shelf("shelf_01"),
                    revision,
                    "fill shelf",
                ),
                CommandResult::Applied { .. }
            ));
        }
        let before = draft.clone();
        let overflow = [PlacementChange::Add {
            placement_id: None,
            product_id: ProductId::new("jif_crunchy_16"),
            shelf_id: shelf("shelf_01"),
            sequence: 0,
            resolved_x: None,
            facings_x: Some(1),
            facings_y: Some(1),
            facings_z: Some(1),
        }];

        let preview = draft.preview_placement_changes(&version, &overflow, 12);
        assert!(matches!(
            preview,
            PreviewResult::ValidationFailed { revision: 12, ref validation }
                if validation.issues.iter().any(|issue| issue.code == ValidationCode::PlacementOutOfBounds)
        ));
        assert_eq!(draft, before);

        let applied = draft.apply_placement_changes_as(
            &version,
            &overflow,
            12,
            "webmcp",
            "Overfill the shelf",
        );
        assert!(matches!(
            applied,
            CommandResult::ValidationFailed { revision: 12, ref validation }
                if validation.issues.iter().any(|issue| issue.code == ValidationCode::PlacementOutOfBounds)
        ));
        assert_eq!(draft, before);
    }

    #[test]
    fn generic_sequence_resolves_even_physical_positions_without_model_coordinates() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        let applied = draft.apply_placement_changes_as(
            &version,
            &[
                PlacementChange::Add {
                    placement_id: None,
                    product_id: ProductId::new("jif_crunchy_16"),
                    shelf_id: shelf("shelf_01"),
                    sequence: 1,
                    resolved_x: None,
                    facings_x: Some(1),
                    facings_y: Some(1),
                    facings_z: Some(1),
                },
                PlacementChange::Add {
                    placement_id: None,
                    product_id: ProductId::new("skippy_chunk_16"),
                    shelf_id: shelf("shelf_01"),
                    sequence: 0,
                    resolved_x: None,
                    facings_x: Some(1),
                    facings_y: Some(1),
                    facings_z: Some(1),
                },
            ],
            0,
            "webmcp",
            "Order the brand block",
        );
        assert!(matches!(
            applied,
            CommandResult::Applied { revision: 1, .. }
        ));
        let skippy = draft
            .placements
            .iter()
            .find(|placement| placement.product_id == ProductId::new("skippy_chunk_16"))
            .unwrap();
        let jif = draft
            .placements
            .iter()
            .find(|placement| placement.product_id == ProductId::new("jif_crunchy_16"))
            .unwrap();
        assert_eq!(skippy.x, Length::ZERO);
        assert_eq!(jif.x, Length::from_sixteenths(62));
        assert_eq!(skippy.x.sixteenths() % 2, 0);
        assert_eq!(jif.x.sixteenths() % 2, 0);
    }

    #[test]
    fn generic_add_reflows_existing_items_and_batch_undo_restores_their_exact_positions() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        draft.add_placement(
            &version,
            &ProductId::new("jif_creamy_16"),
            &shelf("shelf_01"),
            0,
            "existing",
        );
        let existing = draft.placements[0].clone();
        let applied = draft.apply_placement_changes_as(
            &version,
            &[PlacementChange::Add {
                placement_id: None,
                product_id: ProductId::new("skippy_creamy_16"),
                shelf_id: shelf("shelf_01"),
                sequence: 0,
                resolved_x: None,
                facings_x: None,
                facings_y: None,
                facings_z: None,
            }],
            1,
            "webmcp",
            "Prepend the next brand block",
        );
        let change_set_id = match applied {
            CommandResult::Applied { change_set, .. } => {
                assert_eq!(change_set.operations.len(), 2);
                change_set.id
            }
            result => panic!("unexpected result: {result:?}"),
        };
        let moved_existing = draft
            .placements
            .iter()
            .find(|placement| placement.id == existing.id)
            .unwrap();
        assert_eq!(moved_existing.x, Length::from_sixteenths(184));

        let undone = draft.undo_change_set_as(&version, &change_set_id, 2, "webmcp");
        assert!(matches!(undone, CommandResult::Applied { revision: 3, .. }));
        assert_eq!(draft.placements, vec![existing]);
    }

    #[test]
    fn placement_removal_is_revisioned_atomic_and_exactly_undoable() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        draft.add_placement(
            &version,
            &ProductId::new("jif_creamy_16"),
            &shelf("shelf_01"),
            0,
            "catalog add",
        );
        let original = draft.placements[0].clone();

        let stale = draft.remove_placement(&version, &original.id, 0, "stale remove");
        assert!(matches!(stale, CommandResult::RevisionConflict { .. }));
        assert_eq!(draft.placements, vec![original.clone()]);
        assert_eq!(draft.revision, 1);

        let removed = draft.remove_placement(&version, &original.id, 1, "inspector remove");
        let removal_change = match removed {
            CommandResult::Applied {
                revision,
                change_set,
                ref scene_patch,
                ..
            } => {
                assert_eq!(revision, 2);
                assert_eq!(scene_patch.removed_placement_ids, vec![original.id.clone()]);
                change_set.id
            }
            result => panic!("unexpected result: {result:?}"),
        };
        assert!(draft.placements.is_empty());

        let undone = draft.undo_change_set(&version, &removal_change, 2);
        assert!(matches!(undone, CommandResult::Applied { revision: 3, .. }));
        assert_eq!(draft.placements, vec![original]);
    }

    #[test]
    fn placement_move_uses_eighth_inch_grid_records_one_operation_and_undoes_exactly() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        draft.add_placement(
            &version,
            &ProductId::new("jif_creamy_16"),
            &shelf("shelf_01"),
            0,
            "catalog add",
        );
        let original = draft.placements[0].clone();

        let moved = draft.move_placement(
            &version,
            &original.id,
            &shelf("shelf_02"),
            Length::from_sixteenths(2),
            1,
            "Inspector move",
        );
        let move_change = match moved {
            CommandResult::Applied {
                revision,
                change_set,
                ref scene_patch,
                ..
            } => {
                assert_eq!(revision, 2);
                assert_eq!(scene_patch.placements.len(), 1);
                assert_eq!(scene_patch.placements[0].shelf_id, shelf("shelf_02"));
                assert_eq!(scene_patch.placements[0].x, Length::from_sixteenths(2));
                match &change_set.operations[0] {
                    PlanogramOperation::MovePlacement(operation) => {
                        assert_eq!(operation.placement_id, original.id);
                        assert_eq!(operation.before.shelf_id, shelf("shelf_01"));
                        assert_eq!(operation.after.shelf_id, shelf("shelf_02"));
                        assert_eq!(operation.before.x, Length::ZERO);
                        assert_eq!(operation.after.x, Length::from_sixteenths(2));
                    }
                    operation => panic!("unexpected operation: {operation:?}"),
                }
                assert_eq!(change_set.operations.len(), 1);
                change_set.id
            }
            result => panic!("unexpected result: {result:?}"),
        };
        assert_eq!(draft.revision, 2);
        assert_eq!(draft.change_sets.len(), 2);
        assert_eq!(draft.placements[0].shelf_id, shelf("shelf_02"));
        assert_eq!(draft.placements[0].x, Length::from_sixteenths(2));

        let undone = draft.undo_change_set(&version, &move_change, 2);
        assert!(matches!(undone, CommandResult::Applied { revision: 3, .. }));
        assert_eq!(draft.placements[0].shelf_id, original.shelf_id);
        assert_eq!(draft.placements[0].x, original.x);
        assert_eq!(draft.change_sets.len(), 3);
        assert_eq!(draft.change_sets[2].compensates, Some(move_change));
    }

    #[test]
    fn invalid_placement_moves_are_atomic_for_increment_and_shelf_bounds() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        draft.add_placement(
            &version,
            &ProductId::new("jif_creamy_16"),
            &shelf("shelf_01"),
            0,
            "catalog add",
        );
        let placement = draft.placements[0].clone();

        for (target_x, code) in [
            (1, ValidationCode::PlacementXIncrement),
            (712, ValidationCode::PlacementOutOfBounds),
        ] {
            let before = draft.clone();
            let result = draft.move_placement(
                &version,
                &placement.id,
                &shelf("shelf_01"),
                Length::from_sixteenths(target_x),
                1,
                "Invalid move",
            );
            assert!(matches!(
                result,
                CommandResult::ValidationFailed { ref validation, revision: 1 }
                    if validation.issues.iter().any(|issue| issue.code == code)
            ));
            assert_eq!(draft, before);
        }

        let before = draft.clone();
        let result = draft.move_placement(
            &version,
            &placement.id,
            &shelf("base_deck"),
            Length::ZERO,
            1,
            "fixed deck",
        );
        assert!(matches!(
            result,
            CommandResult::ValidationFailed { ref validation, revision: 1 }
                if validation.issues.iter().any(|issue| issue.code == ValidationCode::PlacementOnFixedShelf)
        ));
        assert_eq!(draft, before);
    }

    #[test]
    fn invalid_placement_moves_block_overlap_depth_and_clearance() {
        let mut overlap = DraftVersion::default();
        let version = overlap.id.clone();
        overlap.add_placement(
            &version,
            &ProductId::new("jif_creamy_16"),
            &shelf("shelf_01"),
            0,
            "first",
        );
        overlap.add_placement(
            &version,
            &ProductId::new("skippy_creamy_16"),
            &shelf("shelf_01"),
            1,
            "second",
        );
        let second = overlap.placements[1].clone();
        let before = overlap.clone();
        let result = overlap.move_placement(
            &version,
            &second.id,
            &shelf("shelf_01"),
            Length::ZERO,
            2,
            "overlap",
        );
        assert!(matches!(
            result,
            CommandResult::ValidationFailed { ref validation, .. }
                if validation.issues.iter().any(|issue| issue.code == ValidationCode::PlacementOverlap)
        ));
        assert_eq!(overlap, before);

        let before = overlap.clone();
        let result = overlap.move_placement(
            &version,
            &second.id,
            &shelf("shelf_01"),
            Length::from_sixteenths(176),
            2,
            "too close",
        );
        assert!(matches!(
            result,
            CommandResult::ValidationFailed { ref validation, .. }
                if validation.issues.iter().any(|issue| issue.code == ValidationCode::PlacementGap)
        ));
        assert_eq!(overlap, before);

        let mut depth = DraftVersion::default();
        let version = depth.id.clone();
        depth.fixture.sections[0]
            .shelves
            .iter_mut()
            .find(|candidate| candidate.id == shelf("shelf_02"))
            .unwrap()
            .depth = Length::from_sixteenths(56);
        depth.add_placement(
            &version,
            &ProductId::new("jif_creamy_16"),
            &shelf("shelf_01"),
            0,
            "first",
        );
        let placement = depth.placements[0].clone();
        let before = depth.clone();
        let result = depth.move_placement(
            &version,
            &placement.id,
            &shelf("shelf_02"),
            Length::ZERO,
            1,
            "depth",
        );
        assert!(matches!(
            result,
            CommandResult::ValidationFailed { ref validation, .. }
                if validation.issues.iter().any(|issue| issue.code == ValidationCode::PlacementTooDeep)
        ));
        assert_eq!(depth, before);

        let mut clearance = DraftVersion::default();
        let version = clearance.id.clone();
        clearance.fixture.sections[0]
            .shelves
            .iter_mut()
            .find(|candidate| candidate.id == shelf("shelf_02"))
            .unwrap()
            .elevation = Length::from_sixteenths(500);
        clearance.fixture.sections[0]
            .shelves
            .iter_mut()
            .find(|candidate| candidate.id == shelf("shelf_03"))
            .unwrap()
            .elevation = Length::from_sixteenths(550);
        clearance.add_placement(
            &version,
            &ProductId::new("jif_creamy_40"),
            &shelf("shelf_01"),
            0,
            "first",
        );
        let placement = clearance.placements[0].clone();
        let before = clearance.clone();
        let result = clearance.move_placement(
            &version,
            &placement.id,
            &shelf("shelf_02"),
            Length::ZERO,
            1,
            "clearance",
        );
        assert!(matches!(
            result,
            CommandResult::ValidationFailed { ref validation, .. }
                if validation.issues.iter().any(|issue| issue.code == ValidationCode::PlacementTooTall)
        ));
        assert_eq!(clearance, before);
    }

    #[test]
    fn placement_move_rejects_stale_and_published_commands_without_mutation() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        draft.add_placement(
            &version,
            &ProductId::new("jif_creamy_16"),
            &shelf("shelf_01"),
            0,
            "first",
        );
        let placement = draft.placements[0].clone();
        let before = draft.clone();
        let stale = draft.move_placement(
            &version,
            &placement.id,
            &shelf("shelf_02"),
            Length::from_sixteenths(2),
            0,
            "stale",
        );
        assert!(matches!(stale, CommandResult::RevisionConflict { .. }));
        assert_eq!(draft, before);

        draft.status = VersionStatus::Published;
        let before = draft.clone();
        let forbidden = draft.move_placement(
            &version,
            &placement.id,
            &shelf("shelf_02"),
            Length::from_sixteenths(2),
            1,
            "published",
        );
        assert!(matches!(forbidden, CommandResult::Forbidden { .. }));
        assert_eq!(draft, before);
    }

    #[test]
    fn placement_move_rejects_missing_version_placement_shelf_and_product_ids() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        let missing_placement = draft.move_placement(
            &version,
            &PlacementId::new("missing"),
            &shelf("shelf_01"),
            Length::ZERO,
            0,
            "missing placement",
        );
        assert!(matches!(
            missing_placement,
            CommandResult::NotFound { ref entity, .. } if entity == "placement"
        ));
        let wrong_version = draft.move_placement(
            &VersionId::new("wrong"),
            &PlacementId::new("missing"),
            &shelf("shelf_01"),
            Length::ZERO,
            0,
            "wrong version",
        );
        assert!(matches!(
            wrong_version,
            CommandResult::NotFound { ref entity, .. } if entity == "version"
        ));

        draft.add_placement(
            &version,
            &ProductId::new("jif_creamy_16"),
            &shelf("shelf_01"),
            0,
            "first",
        );
        let placement = draft.placements[0].clone();
        let missing_shelf = draft.move_placement(
            &version,
            &placement.id,
            &shelf("missing"),
            Length::ZERO,
            1,
            "missing shelf",
        );
        assert!(matches!(
            missing_shelf,
            CommandResult::NotFound { ref entity, .. } if entity == "shelf"
        ));

        draft
            .products
            .retain(|product| product.id != placement.product_id);
        let missing_product = draft.move_placement(
            &version,
            &placement.id,
            &shelf("shelf_02"),
            Length::ZERO,
            1,
            "missing product",
        );
        assert!(matches!(
            missing_product,
            CommandResult::NotFound { ref entity, .. } if entity == "product"
        ));
        assert_eq!(draft.revision, 1);
        assert_eq!(draft.change_sets.len(), 1);
    }

    #[test]
    fn missing_placement_removal_does_not_mutate_state() {
        let mut draft = DraftVersion::default();
        let before = draft.clone();
        let result = draft.remove_placement(
            &draft.id.clone(),
            &PlacementId::new("missing"),
            0,
            "remove missing",
        );
        assert!(matches!(
            result,
            CommandResult::NotFound { ref entity, .. } if entity == "placement"
        ));
        assert_eq!(draft, before);
    }

    #[test]
    fn undo_restores_legacy_odd_position_without_rounding() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        draft.add_placement(
            &version,
            &ProductId::new("jif_crunchy_16"),
            &shelf("shelf_01"),
            0,
            "first",
        );
        draft.add_placement(
            &version,
            &ProductId::new("skippy_chunk_16"),
            &shelf("shelf_01"),
            1,
            "second",
        );
        draft.placements[1].x = Length::from_sixteenths(59);
        let removed_id = draft.placements[1].id.clone();
        let removed_x = draft.placements[1].x;
        let removed = draft.remove_placement(&version, &removed_id, 2, "remove");
        let change_set_id = match removed {
            CommandResult::Applied { change_set, .. } => change_set.id,
            result => panic!("unexpected result: {result:?}"),
        };

        let undone = draft.undo_change_set(&version, &change_set_id, 3);
        assert!(matches!(undone, CommandResult::Applied { revision: 4, .. }));
        assert_eq!(draft.placement(&removed_id).unwrap().x, removed_x);
    }

    #[test]
    fn shelf_distribution_is_atomic_grid_aligned_balanced_and_undoable() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        for (revision, product_id) in ["jif_creamy_16", "skippy_creamy_16", "peter_pan_creamy_16"]
            .into_iter()
            .enumerate()
        {
            assert!(matches!(
                draft.add_placement(
                    &version,
                    &ProductId::new(product_id),
                    &shelf("shelf_01"),
                    revision as u64,
                    "catalog add",
                ),
                CommandResult::Applied { .. }
            ));
        }
        let before = draft.placements.clone();
        let result = draft.distribute_shelf(
            &version,
            &shelf("shelf_01"),
            ShelfDistribution::SpaceEvenly,
            3,
            "inspector space evenly",
        );
        let change_set_id = match result {
            CommandResult::Applied {
                revision,
                change_set,
                ref scene_patch,
                ..
            } => {
                assert_eq!(revision, 4);
                assert_eq!(change_set.operations.len(), 3);
                assert_eq!(scene_patch.placements.len(), 3);
                change_set.id
            }
            result => panic!("unexpected result: {result:?}"),
        };

        let mut ordered = draft.placements.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|placement| placement.x);
        assert_eq!(
            ordered
                .iter()
                .map(|placement| placement.id.clone())
                .collect::<Vec<_>>(),
            before
                .iter()
                .map(|placement| placement.id.clone())
                .collect::<Vec<_>>()
        );
        assert!(ordered
            .iter()
            .all(|placement| placement.x.sixteenths() % 2 == 0));
        for pair in ordered.windows(2) {
            let left = pair[0];
            let right = pair[1];
            let product = draft
                .products
                .iter()
                .find(|product| product.id == left.product_id)
                .unwrap();
            assert!(
                right.x - (left.x + DraftVersion::display_width(left, product))
                    >= MIN_PLACEMENT_GAP
            );
        }
        let first = ordered[0];
        let last = ordered[ordered.len() - 1];
        let last_product = draft
            .products
            .iter()
            .find(|product| product.id == last.product_id)
            .unwrap();
        let left_margin = first.x.sixteenths();
        let right_margin = (draft.shelf(&shelf("shelf_01")).unwrap().width
            - (last.x + DraftVersion::display_width(last, last_product)))
        .sixteenths();
        assert!((left_margin - right_margin).abs() <= 3);

        let undone = draft.undo_change_set(&version, &change_set_id, 4);
        assert!(matches!(undone, CommandResult::Applied { revision: 5, .. }));
        assert_eq!(draft.placements, before);
    }

    #[test]
    fn distribution_modes_resolve_deterministically_on_the_eighth_inch_grid() {
        let widths = [
            Length::from_sixteenths(57),
            Length::from_sixteenths(60),
            Length::from_sixteenths(55),
        ];
        let shelf_width = Length::from_sixteenths(768);
        for distribution in [
            ShelfDistribution::PackedLeft,
            ShelfDistribution::Centered,
            ShelfDistribution::SpaceBetween,
            ShelfDistribution::SpaceEvenly,
        ] {
            let first = resolve_shelf_distribution(&widths, shelf_width, distribution).unwrap();
            let second = resolve_shelf_distribution(&widths, shelf_width, distribution).unwrap();
            assert_eq!(first, second);
            assert!(first.iter().all(|position| position.sixteenths() % 2 == 0));
            for index in 1..first.len() {
                assert!(first[index] - (first[index - 1] + widths[index - 1]) >= MIN_PLACEMENT_GAP);
            }
            assert!(*first.last().unwrap() + widths[widths.len() - 1] <= shelf_width);
        }
    }

    #[test]
    fn duplicate_placement_targets_are_rejected_atomically() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        draft.add_placement(
            &version,
            &ProductId::new("jif_creamy_16"),
            &shelf("shelf_01"),
            0,
            "first",
        );
        let placement_id = draft.placements[0].id.clone();
        let before = draft.clone();
        let result = draft.preview_placement_changes(
            &version,
            &[
                PlacementChange::Move {
                    placement_id: placement_id.clone(),
                    shelf_id: shelf("shelf_02"),
                    sequence: 0,
                    resolved_x: None,
                },
                PlacementChange::Remove { placement_id },
            ],
            1,
        );
        assert!(matches!(
            result,
            PreviewResult::InvalidCommand { ref message }
                if message.contains("may only appear once")
        ));
        assert_eq!(draft, before);
    }

    #[test]
    fn catalog_has_complete_fixed_point_metrics_and_exact_five_loaded_trays() {
        let draft = DraftVersion::default();
        let products = &draft.products;
        assert_eq!(products.len(), 22);
        assert!(draft.validate_planogram().valid);
        assert!(products.iter().all(|product| {
            product.dimensions.depth > Length::ZERO
                && product.net_weight_ounces_hundredths > 0
                && product.casepack_quantity > 0
                && product.performance.sales_per_store_per_week_cents > 0
                && product.performance.units_per_store_per_week_milliunits > 0
                && product.performance.gross_margin_basis_points > 0
                && product.performance.source == PERFORMANCE_SOURCE
                && product.performance.period == PERFORMANCE_PERIOD
        }));
        assert_eq!(
            products
                .iter()
                .map(|product| (
                    product.id.0.as_str(),
                    product.net_weight_ounces_hundredths,
                    product.performance.sales_per_store_per_week_cents,
                    product.performance.units_per_store_per_week_milliunits,
                    product.performance.gross_margin_basis_points,
                    product.casepack_quantity,
                ))
                .collect::<Vec<_>>(),
            vec![
                ("jif_creamy_16", 1_600, 3_665, 10_500, 2_850, 12),
                ("jif_crunchy_16", 1_600, 2_024, 5_800, 2_875, 12),
                ("jif_natural_16", 1_600, 1_716, 4_300, 3_100, 12),
                ("jif_creamy_40", 4_000, 3_895, 5_200, 2_550, 6),
                ("jif_crunchy_40", 4_000, 1_947, 2_600, 2_575, 6),
                ("jif_natural_40", 4_000, 1_678, 2_100, 2_800, 6),
                ("skippy_creamy_16", 1_630, 2_928, 8_900, 2_900, 12),
                ("skippy_chunk_16", 1_630, 1_546, 4_700, 2_925, 12),
                ("skippy_natural_16", 1_500, 1_364, 3_600, 3_150, 12),
                ("skippy_creamy_40", 4_000, 3_076, 4_400, 2_600, 6),
                ("skippy_chunk_40", 4_000, 1_538, 2_200, 2_625, 6),
                ("skippy_natural_40", 4_000, 1_348, 1_800, 2_850, 6),
                ("peter_pan_creamy_16", 1_630, 1_914, 6_400, 2_750, 12),
                ("peter_pan_crunchy_16", 1_630, 927, 3_100, 2_775, 12),
                ("peter_pan_creamy_40", 4_000, 1_947, 3_000, 2_500, 6),
                ("peter_pan_crunchy_40", 4_000, 909, 1_400, 2_525, 6),
                ("smuckers_natural_16", 1_600, 1_572, 3_500, 3_300, 12),
                ("smuckers_chunky_16", 1_600, 808, 1_800, 3_325, 12),
                ("smuckers_natural_26", 2_600, 1_298, 2_000, 3_100, 6),
                ("smuckers_chunky_26", 2_600, 649, 1_000, 3_125, 6),
                ("justins_classic_16", 1_600, 1_118, 1_600, 3_600, 6),
                ("justins_classic_28", 2_800, 879, 800, 3_400, 6),
            ]
        );
        assert_eq!(
            products
                .iter()
                .filter_map(|product| {
                    let tray = product.tray.as_ref()?;
                    Some((
                        product.id.0.as_str(),
                        tray.facings_x,
                        tray.units_deep,
                        tray.outer_width.sixteenths(),
                        tray.outer_height.sixteenths(),
                        tray.outer_depth.sixteenths(),
                        tray.front_lip_height.sixteenths(),
                    ))
                })
                .collect::<Vec<_>>(),
            vec![
                ("jif_creamy_16", 3, 4, 175, 80, 232, 20),
                ("skippy_creamy_16", 3, 4, 181, 78, 240, 20),
                ("peter_pan_creamy_16", 3, 4, 178, 78, 236, 20),
                ("smuckers_natural_16", 2, 3, 116, 84, 172, 20),
                ("justins_classic_16", 2, 3, 116, 86, 172, 20),
            ]
        );
    }

    #[test]
    fn tray_direct_add_uses_one_loaded_footprint_everywhere() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        let result = draft.add_placement(
            &version,
            &ProductId::new("jif_creamy_16"),
            &shelf("shelf_01"),
            0,
            "stock loaded tray",
        );
        let change_set_id = match result {
            CommandResult::Applied {
                revision: 1,
                change_set,
                ref scene_patch,
                ..
            } => {
                let node = &scene_patch.placements[0];
                assert_eq!(node.width, Length::from_sixteenths(175));
                assert_eq!(node.height, Length::from_sixteenths(80));
                assert_eq!(node.required_depth, Length::from_sixteenths(232));
                assert_eq!(node.stocking_mode, StockingMode::Tray);
                assert_eq!(node.stocked_unit_count, 12);
                assert_eq!((node.facings_x, node.facings_y, node.facings_z), (3, 1, 4));
                change_set.id
            }
            result => panic!("unexpected result: {result:?}"),
        };
        let placement = draft.placements[0].clone();
        let view = draft.placement_view(&placement.id).unwrap();
        assert_eq!(view.id, placement.id);
        assert_eq!(view.product_id, ProductId::new("jif_creamy_16"));
        assert_eq!(view.stocking_mode, StockingMode::Tray);
        assert_eq!(view.stocked_unit_count, 12);
        assert_eq!(view.geometry.display_width, Length::from_sixteenths(175));

        draft.shelf_mut(&shelf("shelf_02")).unwrap().depth = Length::from_sixteenths(230);
        let before = draft.clone();
        let rejected = draft.move_placement(
            &version,
            &placement.id,
            &shelf("shelf_02"),
            Length::ZERO,
            1,
            "tray depth overflow",
        );
        assert!(matches!(
            rejected,
            CommandResult::ValidationFailed { revision: 1, ref validation }
                if validation.issues.iter().any(|issue| issue.code == ValidationCode::PlacementTooDeep)
        ));
        assert_eq!(draft, before);

        let undone = draft.undo_change_set(&version, &change_set_id, 1);
        assert!(matches!(undone, CommandResult::Applied { revision: 2, .. }));
        assert!(draft.placements.is_empty());
    }

    #[test]
    fn tray_proposals_resolve_omitted_facings_and_reject_explicit_conflicts_atomically() {
        let mut draft = DraftVersion::default();
        let version = draft.id.clone();
        let conflicting = [PlacementChange::Add {
            placement_id: None,
            product_id: ProductId::new("jif_creamy_16"),
            shelf_id: shelf("shelf_01"),
            sequence: 0,
            resolved_x: None,
            facings_x: Some(1),
            facings_y: Some(1),
            facings_z: Some(1),
        }];
        let before = draft.clone();
        let rejected =
            draft.apply_placement_changes(&version, &conflicting, 0, "conflicting tray facings");
        assert!(matches!(
            rejected,
            CommandResult::ValidationFailed { revision: 0, ref validation }
                if validation.issues.iter().any(|issue| issue.code == ValidationCode::TrayFacingMismatch)
        ));
        assert_eq!(draft, before);

        let omitted = [PlacementChange::Add {
            placement_id: None,
            product_id: ProductId::new("jif_creamy_16"),
            shelf_id: shelf("shelf_01"),
            sequence: 0,
            resolved_x: None,
            facings_x: None,
            facings_y: None,
            facings_z: None,
        }];
        assert!(matches!(
            draft.preview_placement_changes(&version, &omitted, 0),
            PreviewResult::Ready { revision: 0, .. }
        ));
        let applied = draft.apply_placement_changes(&version, &omitted, 0, "stock tray");
        assert!(matches!(
            applied,
            CommandResult::Applied { revision: 1, .. }
        ));
        assert_eq!(
            (
                draft.placements[0].facings_x,
                draft.placements[0].facings_y,
                draft.placements[0].facings_z,
            ),
            (3, 1, 4)
        );
    }

    #[test]
    fn catalog_includes_near_eight_inch_family_size_variants_for_every_brand() {
        let products = default_products();
        let tall = products
            .iter()
            .filter(|product| product.dimensions.confidence == "concept")
            .collect::<Vec<_>>();
        assert_eq!(products.len(), 22);
        assert_eq!(tall.len(), 11);
        assert!(tall.iter().all(|product| {
            (Length::from_sixteenths(120)..=Length::inches(8)).contains(&product.dimensions.height)
        }));
        for brand in ["Jif", "SKIPPY", "Peter Pan", "Smucker's", "Justin's"] {
            assert!(tall.iter().any(|product| product.brand == brand));
        }
    }

    #[test]
    fn catalog_uses_one_color_per_brand() {
        let products = default_products();
        for brand in ["Jif", "SKIPPY", "Peter Pan", "Smucker's", "Justin's"] {
            let brand_colors = products
                .iter()
                .filter(|product| product.brand == brand)
                .map(|product| product.color)
                .collect::<std::collections::HashSet<_>>();
            assert_eq!(brand_colors.len(), 1, "{brand} should have one brand color");
        }
    }
}

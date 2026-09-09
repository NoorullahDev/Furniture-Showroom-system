use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDto {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub sort_order: i64,
    pub is_active: bool,
    pub product_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductTypeDto {
    pub id: i64,
    pub category_id: i64,
    pub name: String,
    pub is_active: bool,
    pub product_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitDto {
    pub id: i64,
    pub name: String,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributeInputDto {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductImageDto {
    pub id: i64,
    pub image_path: String,
    pub thumbnail_path: String,
    pub relative_path: String,
    pub thumbnail_relative_path: String,
    pub sort_order: i64,
    pub is_primary: bool,
    pub sha256: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductListItemDto {
    pub id: i64,
    pub article_number: String,
    pub name: String,
    pub category_id: i64,
    pub category: String,
    pub product_type_id: Option<i64>,
    pub product_type: Option<String>,
    pub unit: Option<String>,
    pub sale_price_minor: i64,
    pub cost_minor: Option<i64>,
    pub minimum_stock: i64,
    pub track_stock: bool,
    pub is_active: bool,
    pub archived_at: Option<String>,
    pub primary_thumbnail_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDetailDto {
    pub id: i64,
    pub article_number: String,
    pub name: String,
    pub category_id: i64,
    pub category: String,
    pub product_type_id: Option<i64>,
    pub product_type: Option<String>,
    pub unit_id: Option<i64>,
    pub unit: Option<String>,
    pub description: Option<String>,
    pub material: Option<String>,
    pub color: Option<String>,
    pub dimensions_text: Option<String>,
    pub brand: Option<String>,
    pub barcode: Option<String>,
    pub warranty_months: Option<i64>,
    pub notes: Option<String>,
    pub cost_minor: Option<i64>,
    pub sale_price_minor: i64,
    pub minimum_stock: i64,
    pub track_stock: bool,
    pub is_active: bool,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub images: Vec<ProductImageDto>,
    pub attributes: Vec<AttributeInputDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProductInput {
    pub article_number: String,
    pub name: String,
    pub category_id: i64,
    pub product_type_id: Option<i64>,
    pub unit_id: Option<i64>,
    pub description: Option<String>,
    pub material: Option<String>,
    pub color: Option<String>,
    pub dimensions_text: Option<String>,
    pub brand: Option<String>,
    pub barcode: Option<String>,
    pub warranty_months: Option<i64>,
    pub notes: Option<String>,
    pub cost_minor: i64,
    pub sale_price_minor: i64,
    pub minimum_stock: i64,
    pub track_stock: bool,
    pub attributes: Vec<AttributeInputDto>,
    pub image_paths: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProductInput {
    pub article_number: Option<String>,
    pub name: Option<String>,
    pub category_id: Option<i64>,
    pub product_type_id: Option<Option<i64>>,
    pub unit_id: Option<Option<i64>>,
    pub description: Option<String>,
    pub material: Option<String>,
    pub color: Option<String>,
    pub dimensions_text: Option<String>,
    pub brand: Option<String>,
    pub barcode: Option<String>,
    pub warranty_months: Option<i64>,
    pub notes: Option<String>,
    pub cost_minor: Option<i64>,
    pub sale_price_minor: Option<i64>,
    pub minimum_stock: Option<i64>,
    pub track_stock: Option<bool>,
    pub attributes: Option<Vec<AttributeInputDto>>,
}

use super::{ProductsBindingV2, pretty_member_prefix, pretty_member_string, push_indent};
use crate::FormatError;

pub(super) fn write_products(
    output: &mut String,
    products: &ProductsBindingV2,
    indent: usize,
) -> Result<(), FormatError> {
    pretty_member_prefix(output, indent, "products")?;
    output.push_str("{\n");
    pretty_member_string(output, indent + 1, "schema", &products.schema, true)?;
    pretty_member_string(output, indent + 1, "key_field", &products.key_field, true)?;
    pretty_member_string(
        output,
        indent + 1,
        "category_field",
        &products.category_field,
        true,
    )?;
    pretty_member_string(
        output,
        indent + 1,
        "price_field",
        &products.price_field,
        false,
    )?;
    push_indent(output, indent);
    output.push_str("}\n");
    Ok(())
}

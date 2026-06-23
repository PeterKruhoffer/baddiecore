using Vogen;

namespace Baddiecore.Data.Ids;

[ValueObject<string>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class TemplateId
{
    private static Validation Validate(string value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<string>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class TemplateFieldId
{
    private static Validation Validate(string value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<string>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class WorkflowStateId
{
    private static Validation Validate(string value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<string>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class WorkflowActionId
{
    private static Validation Validate(string value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<string>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class WorkflowActionRoleId
{
    private static Validation Validate(string value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<string>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class ContentItemId
{
    private static Validation Validate(string value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<string>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class ContentFieldValueId
{
    private static Validation Validate(string value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<string>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class RenderingDefinitionId
{
    private static Validation Validate(string value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<string>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class RenderingFieldId
{
    private static Validation Validate(string value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<string>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class RenderingAllowedPlaceholderId
{
    private static Validation Validate(string value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<string>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class PageRenderingId
{
    private static Validation Validate(string value) => CmsIdValidation.ValidateId(value);
}

internal static class CmsIdValidation
{
    public static Validation ValidateId(string value)
    {
        return Guid.TryParseExact(value, "D", out _)
            ? Validation.Ok
            : Validation.Invalid("IDs must be canonical GUID strings.");
    }
}

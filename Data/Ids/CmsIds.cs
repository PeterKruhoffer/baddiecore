using Vogen;

namespace Baddiecore.Data.Ids;

[ValueObject<Guid>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class TemplateId
{
    private static Validation Validate(Guid value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<Guid>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class TemplateFieldId
{
    private static Validation Validate(Guid value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<Guid>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class WorkflowStateId
{
    private static Validation Validate(Guid value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<Guid>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class WorkflowActionId
{
    private static Validation Validate(Guid value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<Guid>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class WorkflowActionRoleId
{
    private static Validation Validate(Guid value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<Guid>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class ContentItemId
{
    private static Validation Validate(Guid value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<Guid>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class ContentFieldValueId
{
    private static Validation Validate(Guid value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<Guid>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class RenderingDefinitionId
{
    private static Validation Validate(Guid value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<Guid>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class RenderingFieldId
{
    private static Validation Validate(Guid value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<Guid>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class RenderingAllowedPlaceholderId
{
    private static Validation Validate(Guid value) => CmsIdValidation.ValidateId(value);
}

[ValueObject<Guid>(conversions: Conversions.TypeConverter | Conversions.SystemTextJson | Conversions.EfCoreValueConverter)]
public sealed partial class PageRenderingId
{
    private static Validation Validate(Guid value) => CmsIdValidation.ValidateId(value);
}

internal static class CmsIdValidation
{
    public static Validation ValidateId(Guid value)
    {
        return value != Guid.Empty
            ? Validation.Ok
            : Validation.Invalid("IDs cannot be empty GUIDs.");
    }
}

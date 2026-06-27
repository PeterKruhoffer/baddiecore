using Baddiecore.Models;
using FluentValidation;

namespace Baddiecore.Features.Cms;

public sealed class UpdateRenderingDatasourceRequestValidator : AbstractValidator<UpdateRenderingDatasourceRequest>
{
    public UpdateRenderingDatasourceRequestValidator()
    {
        RuleFor(request => request.DatasourceId)
            .NotNull();

        RuleFor(request => request.ExpectedVersion)
            .GreaterThan(0);

        RuleFor(request => request.Fields)
            .NotNull()
            .NotEmpty();

        RuleForEach(request => request.Fields)
            .SetValidator(new UpdateFieldValueRequestValidator());

        RuleFor(request => request.Fields)
            .Must(fields => fields is null || fields
                .Select(field => field.Name)
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .Count() == fields.Count)
            .WithMessage("Field names must be unique.");
    }
}

public sealed class UpdateFieldValueRequestValidator : AbstractValidator<UpdateFieldValueRequest>
{
    public UpdateFieldValueRequestValidator()
    {
        RuleFor(field => field.Name)
            .NotEmpty()
            .MaximumLength(128);

        RuleFor(field => field.Value)
            .NotNull();
    }
}

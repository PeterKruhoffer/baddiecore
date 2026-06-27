using Baddiecore.Models;
using FluentValidation;

namespace Baddiecore.Features.Cms;

public sealed class GetLayoutRequestValidator : AbstractValidator<GetLayoutRequest>
{
    public GetLayoutRequestValidator()
    {
        RuleFor(request => request.Path)
            .NotEmpty()
            .MaximumLength(512);

        RuleFor(request => request.Mode)
            .NotEmpty()
            .Must(CmsLayoutModes.IsKnown)
            .WithMessage("Layout mode must be either 'preview' or 'published'.");
    }
}

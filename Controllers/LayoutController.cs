using Baddiecore.Features.Cms;
using Baddiecore.Models;
using FluentValidation;
using Microsoft.AspNetCore.Mvc;

namespace Baddiecore.Controllers;

[ApiController]
[Route("api/layout")]
public sealed class LayoutController(
    ICmsQueries cmsQueries,
    IValidator<GetLayoutRequest> validator) : ControllerBase
{
    [HttpGet]
    public async Task<ActionResult<LayoutResponse>> GetLayout(
        [FromQuery] GetLayoutRequest request,
        CancellationToken cancellationToken = default)
    {
        var validationResult = await validator.ValidateAsync(request, cancellationToken);

        if (!validationResult.IsValid)
        {
            return ValidationProblem(new ValidationProblemDetails(validationResult.ToDictionary()));
        }

        var layout = await cmsQueries.GetLayoutAsync(request.Path, request.Mode, cancellationToken);

        if (layout is null)
        {
            return NotFound(new
            {
                Message = $"No layout exists for path '{request.Path}'."
            });
        }

        return Ok(layout);
    }
}

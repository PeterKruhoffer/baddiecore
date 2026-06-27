using Baddiecore.Data.Ids;
using Baddiecore.Features.Cms;
using Baddiecore.Models;
using FluentValidation;
using Microsoft.AspNetCore.Mvc;

namespace Baddiecore.Controllers;

[ApiController]
[Route("api/renderings/{renderingId}/datasource")]
public sealed class RenderingDatasourceController(
    IRenderingDatasourceWriter datasourceWriter,
    IValidator<UpdateRenderingDatasourceRequest> validator) : ControllerBase
{
    [HttpPut("fields")]
    public async Task<IActionResult> UpdateFields(
        PageRenderingId renderingId,
        [FromBody] UpdateRenderingDatasourceRequest request,
        CancellationToken cancellationToken)
    {
        var validationResult = await validator.ValidateAsync(request, cancellationToken);
        if (!validationResult.IsValid)
        {
            return ValidationProblem(new ValidationProblemDetails(validationResult.ToDictionary()));
        }

        return await datasourceWriter.UpdateAsync(renderingId, request, cancellationToken) switch
        {
            UpdateDatasourceSuccess => NoContent(),
            UpdateDatasourceNotFound notFound => NotFound(new ProblemDetails
            {
                Title = "Rendering datasource not found",
                Detail = notFound.Message,
                Status = StatusCodes.Status404NotFound
            }),
            UpdateDatasourceValidation validation => BadRequest(
                new ValidationProblemDetails(validation.Errors.ToDictionary())),
            UpdateDatasourceConflict conflict => Conflict(new ProblemDetails
            {
                Title = "Datasource version conflict",
                Detail = "The datasource changed after it was loaded. Refresh the layout before saving again.",
                Status = StatusCodes.Status409Conflict,
                Extensions = { ["currentVersion"] = conflict.CurrentVersion }
            }),
            _ => throw new InvalidOperationException("Unknown datasource update result.")
        };
    }
}

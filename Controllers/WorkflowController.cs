using Baddiecore.Features.Cms;
using Baddiecore.Models;
using Microsoft.AspNetCore.Mvc;

namespace Baddiecore.Controllers;

[ApiController]
[Route("api/workflow")]
public sealed class WorkflowController(ICmsQueries cmsQueries) : ControllerBase
{
    [HttpGet]
    public async Task<ActionResult<WorkflowResponse>> GetWorkflow(CancellationToken cancellationToken)
    {
        return Ok(await cmsQueries.GetWorkflowAsync(cancellationToken));
    }
}

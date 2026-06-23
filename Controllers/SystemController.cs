using Microsoft.AspNetCore.Mvc;

namespace Baddiecore.Controllers;

[ApiController]
[Route("api/system")]
public sealed class SystemController : ControllerBase
{
    [HttpGet("health")]
    public ActionResult<SystemHealthResponse> GetHealth()
    {
        return Ok(new SystemHealthResponse("ok", "Baddiecore API is running."));
    }
}

public sealed record SystemHealthResponse(string Status, string Message);

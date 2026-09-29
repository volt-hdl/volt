# PSScriptAnalyzer settings for install.ps1 (install.yml runs it; any
# finding fails the job).
@{
    ExcludeRules = @(
        # The installer talks to a person at a console. Write-Host (since
        # PowerShell 5 a wrapper around the information stream) keeps its
        # messages out of the pipeline; Write-Output would turn them into
        # return values of the helper functions, and Write-Information is
        # hidden by default.
        'PSAvoidUsingWriteHost'
    )
}

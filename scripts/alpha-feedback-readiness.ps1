[CmdletBinding()]
param(
  [string]$EvidenceDirectory = (Join-Path (Resolve-Path (Join-Path $PSScriptRoot '..')).Path 'alpha-evidence'),
  [string]$OutputPath = '',
  [switch]$AllowSyntheticFixture
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$script:Failures = [System.Collections.Generic.List[string]]::new()

function Add-Failure {
  param([Parameter(Mandatory = $true)][string]$Message)
  $script:Failures.Add($Message)
}

function Require-Condition {
  param(
    [Parameter(Mandatory = $true)][bool]$Condition,
    [Parameter(Mandatory = $true)][string]$Message
  )
  if (-not $Condition) {
    Add-Failure $Message
  }
}

function Read-EvidenceJson {
  param([Parameter(Mandatory = $true)][string]$Name)

  $path = Join-Path $EvidenceDirectory $Name
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
    throw "Missing required evidence file: $Name"
  }

  try {
    return Get-Content -LiteralPath $path -Raw | ConvertFrom-Json -Depth 100
  }
  catch {
    throw "Invalid JSON in $Name`: $($_.Exception.Message)"
  }
}

function Has-Property {
  param(
    [AllowNull()][object]$Value,
    [Parameter(Mandatory = $true)][string]$Name
  )
  return $null -ne $Value -and $Value.PSObject.Properties.Name -contains $Name
}

function Assert-AllowedProperties {
  param(
    [Parameter(Mandatory = $true)][object]$Value,
    [Parameter(Mandatory = $true)][string[]]$Allowed,
    [Parameter(Mandatory = $true)][string]$Path
  )

  foreach ($property in $Value.PSObject.Properties) {
    if ($Allowed -notcontains $property.Name) {
      Add-Failure "Unexpected field '$($property.Name)' found at $Path. The Alpha evidence schema is fail-closed."
    }
  }
}

function Test-SafeEvidenceValue {
  param(
    [AllowNull()][object]$Value,
    [Parameter(Mandatory = $true)][string]$Path
  )

  if ($null -eq $Value) {
    return
  }

  if ($Value -is [string]) {
    if ($Value -match '(?i)\bBearer\s+[A-Za-z0-9._~-]{8,}' -or
        $Value -match '(?i)\bsk-[A-Za-z0-9_-]{12,}' -or
        $Value -match '(?i)https?://\S+' -or
        $Value -match '(?i)\b[A-Z]:\\[^\s]+' -or
        $Value -match '(?i)/(?:Users|home)/[^\s]+') {
      Add-Failure "Sensitive-looking value is not allowed at $Path."
    }
    return
  }

  if ($Value -is [System.Collections.IDictionary]) {
    foreach ($key in $Value.Keys) {
      Test-SafeEvidenceProperty -Name ([string]$key) -Value $Value[$key] -Path "$Path.$key"
    }
    return
  }

  if ($Value -is [System.Collections.IEnumerable]) {
    $index = 0
    foreach ($item in $Value) {
      Test-SafeEvidenceValue -Value $item -Path "$Path[$index]"
      $index += 1
    }
    return
  }

  foreach ($property in $Value.PSObject.Properties) {
    Test-SafeEvidenceProperty -Name $property.Name -Value $property.Value -Path "$Path.$($property.Name)"
  }
}

function Test-SafeEvidenceProperty {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [AllowNull()][object]$Value,
    [Parameter(Mandatory = $true)][string]$Path
  )

  $normalized = ($Name -replace '[-_\s]', '').ToLowerInvariant()
  $forbiddenNames = @(
    'apikey', 'accesstoken', 'refreshtoken', 'authorization', 'cookie', 'session',
    'password', 'secret', 'body', 'content', 'prompt', 'sourcesnapshot', 'embedding',
    'url', 'query', 'absolutepath', 'filepath', 'email', 'displayname', 'fullname', 'realname'
  )

  if ($forbiddenNames -contains $normalized) {
    Add-Failure "Forbidden evidence field '$Name' found at $Path."
    return
  }

  Test-SafeEvidenceValue -Value $Value -Path $Path
}

function Count-Status {
  param(
    [object[]]$Records,
    [Parameter(Mandatory = $true)][string]$Step,
    [Parameter(Mandatory = $true)][string]$Status
  )
  return @($Records | Where-Object {
    (Has-Property $_ 'funnel') -and
    (Has-Property $_.funnel $Step) -and
    $_.funnel.$Step -eq $Status
  }).Count
}

try {
  $null = Resolve-Path -LiteralPath $EvidenceDirectory
  $participants = Read-EvidenceJson 'participants.json'
  $issues = Read-EvidenceJson 'issues.json'
  $decision = Read-EvidenceJson 'decision.json'

  Test-SafeEvidenceValue -Value $participants -Path 'participants'
  Test-SafeEvidenceValue -Value $issues -Path 'issues'
  Test-SafeEvidenceValue -Value $decision -Path 'decision'
  Assert-AllowedProperties $participants @('schemaVersion', 'evidenceKind', 'releaseCommit', 'records') 'participants'
  Assert-AllowedProperties $issues @('schemaVersion', 'releaseCommit', 'issues') 'issues'
  Assert-AllowedProperties $decision @('schemaVersion', 'releaseCommit', 'decisionDate', 'participantsCompleted', 'primaryPriority', 'evidenceSummary', 'explicitlyNotDoing', 'owners') 'decision'

  foreach ($document in @($participants, $issues, $decision)) {
    Require-Condition (Has-Property $document 'schemaVersion') 'Every evidence document must declare schemaVersion.'
    if (Has-Property $document 'schemaVersion') {
      Require-Condition ($document.schemaVersion -eq 1) 'Only Alpha evidence schemaVersion 1 is supported.'
    }
    Require-Condition (Has-Property $document 'releaseCommit') 'Every evidence document must declare releaseCommit.'
  }

  $releaseCommits = @(@($participants.releaseCommit, $issues.releaseCommit, $decision.releaseCommit) | Select-Object -Unique)
  Require-Condition ($releaseCommits.Count -eq 1) 'All evidence documents must reference the same releaseCommit.'

  Require-Condition (Has-Property $participants 'evidenceKind') 'participants.json must declare evidenceKind.'
  $evidenceKind = if (Has-Property $participants 'evidenceKind') { [string]$participants.evidenceKind } else { '' }
  Require-Condition (@('alpha_observation', 'synthetic_contract_fixture') -contains $evidenceKind) 'evidenceKind must be alpha_observation or synthetic_contract_fixture.'
  if ($evidenceKind -eq 'synthetic_contract_fixture' -and -not $AllowSyntheticFixture) {
    Add-Failure 'Synthetic fixtures cannot satisfy the real Alpha gate. Use real alpha_observation evidence.'
  }
  if ($evidenceKind -eq 'alpha_observation') {
    Require-Condition ([string]$participants.releaseCommit -match '^[0-9a-fA-F]{7,40}$') 'Real Alpha evidence must reference a 7-40 character Git commit SHA.'
  }

  $records = if (Has-Property $participants 'records') { @($participants.records) } else { @() }
  Require-Condition ($records.Count -gt 0) 'participants.json must include records.'
  $participantCodes = @($records | ForEach-Object { if (Has-Property $_ 'participantCode') { $_.participantCode } })
  Require-Condition ($participantCodes.Count -eq (@($participantCodes | Select-Object -Unique)).Count) 'participantCode values must be unique.'

  $invited = @($records | Where-Object { (Has-Property $_ 'invited') -and $_.invited -eq $true })
  $completed = @($records | Where-Object { (Has-Property $_ 'completedObservation') -and $_.completedObservation -eq $true })
  Require-Condition ($invited.Count -ge 5 -and $invited.Count -le 15) 'The invited Alpha cohort must contain 5-15 participants.'
  Require-Condition ($completed.Count -ge 1) 'At least one participant must complete a full observation.'

  foreach ($record in $records) {
    Assert-AllowedProperties $record @('participantCode', 'invited', 'consentConfirmed', 'completedObservation', 'observedOn', 'windowsVersion', 'installResult', 'funnel', 'privacyIncident') 'participants.records[]'
    Require-Condition (Has-Property $record 'participantCode') 'Every participant record needs a participantCode.'
    if (Has-Property $record 'participantCode') {
      Require-Condition ([string]$record.participantCode -match '^alpha-[0-9]{3,}$') 'participantCode must use an anonymous alpha-NNN code.'
    }
    if (Has-Property $record 'windowsVersion') {
      Require-Condition (@('10', '11') -contains [string]$record.windowsVersion) "Participant $($record.participantCode) must use Windows version 10 or 11."
    }
    if (Has-Property $record 'installResult') {
      Require-Condition (@('passed', 'failed', 'not_attempted') -contains $record.installResult) "Participant $($record.participantCode) has an invalid installResult."
    }
    if (Has-Property $record 'funnel') {
      Assert-AllowedProperties $record.funnel @('start', 'save', 'search', 'ai', 'evaluation') 'participants.records[].funnel'
      foreach ($step in @('start', 'save', 'search', 'evaluation')) {
        if (Has-Property $record.funnel $step) {
          Require-Condition (@('passed', 'failed', 'not_attempted') -contains $record.funnel.$step) "Participant $($record.participantCode) has an invalid funnel status for $step."
        }
      }
      if (Has-Property $record.funnel 'ai') {
        Require-Condition (@('passed', 'failed', 'not_configured', 'not_attempted') -contains $record.funnel.ai) "Participant $($record.participantCode) has an invalid AI funnel status."
      }
    }
    Require-Condition ((Has-Property $record 'consentConfirmed') -and $record.consentConfirmed -eq $true) "Consent must be confirmed for participant $($record.participantCode)."
    Require-Condition ((Has-Property $record 'privacyIncident') -and $record.privacyIncident -eq $false) "privacyIncident must be false for participant $($record.participantCode)."
  }

  foreach ($record in $completed) {
    $observedDate = [datetime]::MinValue
    Require-Condition ((Has-Property $record 'observedOn') -and [datetime]::TryParseExact([string]$record.observedOn, 'yyyy-MM-dd', [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::None, [ref]$observedDate)) "Completed participant $($record.participantCode) needs observedOn in yyyy-MM-dd format."
    Require-Condition ((Has-Property $record 'windowsVersion') -and -not [string]::IsNullOrWhiteSpace([string]$record.windowsVersion)) "Completed participant $($record.participantCode) needs a Windows version."
    Require-Condition ((Has-Property $record 'installResult') -and $record.installResult -eq 'passed') "Completed participant $($record.participantCode) must pass installation."
    Require-Condition (Has-Property $record 'funnel') "Completed participant $($record.participantCode) needs funnel evidence."
    if (Has-Property $record 'funnel') {
      foreach ($step in @('start', 'save', 'search', 'evaluation')) {
        Require-Condition ((Has-Property $record.funnel $step) -and $record.funnel.$step -eq 'passed') "Completed participant $($record.participantCode) must pass funnel step '$step'."
      }
      Require-Condition ((Has-Property $record.funnel 'ai') -and @('passed', 'not_configured') -contains $record.funnel.ai) "Completed participant $($record.participantCode) must record AI as passed or not_configured."
    }
  }

  $issueRecords = if (Has-Property $issues 'issues') { @($issues.issues) } else { @() }
  $openIssues = @($issueRecords | Where-Object { (Has-Property $_ 'status') -and $_.status -eq 'open' })
  foreach ($issue in $issueRecords) {
    Assert-AllowedProperties $issue @('id', 'severity', 'status', 'owner', 'workaround', 'dueDate') 'issues.issues[]'
    Require-Condition ((Has-Property $issue 'id') -and -not [string]::IsNullOrWhiteSpace([string]$issue.id)) 'Every issue needs an id.'
    Require-Condition ((Has-Property $issue 'severity') -and @('P0', 'P1', 'P2', 'P3') -contains $issue.severity) "Issue $($issue.id) has an invalid severity."
    Require-Condition ((Has-Property $issue 'status') -and @('open', 'closed') -contains $issue.status) "Issue $($issue.id) has an invalid status."
  }
  $openP0 = @($openIssues | Where-Object { (Has-Property $_ 'severity') -and $_.severity -eq 'P0' })
  $openP1 = @($openIssues | Where-Object { (Has-Property $_ 'severity') -and $_.severity -eq 'P1' })
  Require-Condition ($openP0.Count -eq 0) 'The Alpha gate cannot pass with an open P0.'
  foreach ($issue in $openP1) {
    foreach ($field in @('owner', 'workaround', 'dueDate')) {
      Require-Condition ((Has-Property $issue $field) -and -not [string]::IsNullOrWhiteSpace([string]$issue.$field)) "Open P1 $($issue.id) needs $field."
    }
    if ((Has-Property $issue 'dueDate') -and -not [string]::IsNullOrWhiteSpace([string]$issue.dueDate)) {
      $parsedDate = [datetime]::MinValue
      Require-Condition ([datetime]::TryParse([string]$issue.dueDate, [ref]$parsedDate)) "Open P1 $($issue.id) has an invalid dueDate."
    }
  }

  $decisionDate = [datetime]::MinValue
  Require-Condition ((Has-Property $decision 'decisionDate') -and [datetime]::TryParseExact([string]$decision.decisionDate, 'yyyy-MM-dd', [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::None, [ref]$decisionDate)) 'decision.json must declare decisionDate in yyyy-MM-dd format.'
  Require-Condition (Has-Property $decision 'participantsCompleted') 'decision.json must declare participantsCompleted.'
  if (Has-Property $decision 'participantsCompleted') {
    Require-Condition ($decision.participantsCompleted -eq $completed.Count) 'decision participantsCompleted must match participant evidence.'
  }
  Require-Condition ((Has-Property $decision 'primaryPriority') -and @('evaluation_depth', 'capture_coverage', 'data_safety', 'other') -contains $decision.primaryPriority) 'Decision must select exactly one supported primaryPriority.'
  Require-Condition ((Has-Property $decision 'evidenceSummary') -and ([string]$decision.evidenceSummary).Length -ge 20 -and ([string]$decision.evidenceSummary).Length -le 500) 'Decision evidenceSummary must contain 20-500 redacted characters.'
  Require-Condition ((Has-Property $decision 'owners') -and @($decision.owners).Count -ge 1) 'Decision must name at least one accountable owner code or role.'

  if ($script:Failures.Count -gt 0) {
    Write-Error ("Alpha feedback readiness failed:`n- " + ($script:Failures -join "`n- "))
    exit 1
  }

  $summary = [ordered]@{
    schemaVersion = 1
    status = 'passed'
    evidenceKind = $evidenceKind
    releaseCommit = [string]$participants.releaseCommit
    generatedAt = (Get-Date).ToUniversalTime().ToString('o')
    invitedParticipants = $invited.Count
    completedObservations = $completed.Count
    observationWindow = [ordered]@{
      first = [string](@($completed | ForEach-Object { $_.observedOn } | Sort-Object)[0])
      last = [string](@($completed | ForEach-Object { $_.observedOn } | Sort-Object)[-1])
    }
    decisionDate = [string]$decision.decisionDate
    funnel = [ordered]@{
      startPassed = Count-Status $completed 'start' 'passed'
      savePassed = Count-Status $completed 'save' 'passed'
      searchPassed = Count-Status $completed 'search' 'passed'
      aiPassed = Count-Status $completed 'ai' 'passed'
      aiNotConfigured = Count-Status $completed 'ai' 'not_configured'
      evaluationPassed = Count-Status $completed 'evaluation' 'passed'
    }
    issues = [ordered]@{
      openP0 = $openP0.Count
      openP1 = $openP1.Count
      total = $issueRecords.Count
    }
    primaryPriority = [string]$decision.primaryPriority
  }

  $json = $summary | ConvertTo-Json -Depth 10
  if ($OutputPath) {
    $parent = Split-Path -Parent $OutputPath
    if ($parent -and -not (Test-Path -LiteralPath $parent)) {
      New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    Set-Content -LiteralPath $OutputPath -Value $json -Encoding utf8
  }
  Write-Output $json
}
catch {
  Write-Error $_.Exception.Message
  exit 1
}

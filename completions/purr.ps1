
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'purr' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'purr'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'purr' {
            [CompletionResult]::new('-c', '-c', [CompletionResultType]::ParameterName, 'Path to a custom config file')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'Path to a custom config file')
            [CompletionResult]::new('--ascii_distro', '--ascii_distro', [CompletionResultType]::ParameterName, 'Force a specific distro logo (e.g. "arch")')
            [CompletionResult]::new('--ascii_colors', '--ascii_colors', [CompletionResultType]::ParameterName, 'Override logo colours (space/comma list, e.g. "4 6 1")')
            [CompletionResult]::new('--backend', '--backend', [CompletionResultType]::ParameterName, 'Logo backend: ascii or kitty')
            [CompletionResult]::new('--source', '--source', [CompletionResultType]::ParameterName, 'Image source (PNG) for the kitty backend')
            [CompletionResult]::new('--separator', '--separator', [CompletionResultType]::ParameterName, 'Separator between labels and values')
            [CompletionResult]::new('--underline_char', '--underline_char', [CompletionResultType]::ParameterName, 'Character used for the title underline')
            [CompletionResult]::new('--colors', '--colors', [CompletionResultType]::ParameterName, 'Override text colours (space/comma list)')
            [CompletionResult]::new('--memory_unit', '--memory_unit', [CompletionResultType]::ParameterName, 'Memory unit: kib, mib, or gib')
            [CompletionResult]::new('--uptime_shorthand', '--uptime_shorthand', [CompletionResultType]::ParameterName, 'Uptime format: on, tiny, or off')
            [CompletionResult]::new('--cpu_cores', '--cpu_cores', [CompletionResultType]::ParameterName, 'CPU cores: logical, physical, or off')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Include verbose output or not')
            [CompletionResult]::new('--no-config', '--no-config', [CompletionResultType]::ParameterName, 'Ignore any config file and start from the built-in defaults')
            [CompletionResult]::new('--all', '--all', [CompletionResultType]::ParameterName, 'Use the all-probes preset')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'Use the neofetch text renderer')
            [CompletionResult]::new('--neofetch', '--neofetch', [CompletionResultType]::ParameterName, 'Use the neofetch text renderer')
            [CompletionResult]::new('--json', '--json', [CompletionResultType]::ParameterName, 'Emit JSON instead of text')
            [CompletionResult]::new('--no_ascii_bold', '--no_ascii_bold', [CompletionResultType]::ParameterName, 'Don''t bold the logo')
            [CompletionResult]::new('-L', '-L ', [CompletionResultType]::ParameterName, 'Show only the logo (no info)')
            [CompletionResult]::new('--logo', '--logo', [CompletionResultType]::ParameterName, 'Show only the logo (no info)')
            [CompletionResult]::new('--off', '--off', [CompletionResultType]::ParameterName, 'Hide the logo')
            [CompletionResult]::new('--no_bold', '--no_bold', [CompletionResultType]::ParameterName, 'Don''t bold the title and labels')
            [CompletionResult]::new('--title_fqdn', '--title_fqdn', [CompletionResultType]::ParameterName, 'Show the fully-qualified hostname')
            [CompletionResult]::new('--stdout', '--stdout', [CompletionResultType]::ParameterName, 'Pipe-friendly output: disable colour')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('generate', 'generate', [CompletionResultType]::ParameterValue, 'Generate a new config file')
            [CompletionResult]::new('config-path', 'config-path', [CompletionResultType]::ParameterValue, 'Return default config file path')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'purr;generate' {
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'Generate neofetch preset')
            [CompletionResult]::new('--neofetch', '--neofetch', [CompletionResultType]::ParameterName, 'Generate neofetch preset')
            [CompletionResult]::new('--all', '--all', [CompletionResultType]::ParameterName, 'Use all default presets')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Include verbose output or not')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'purr;config-path' {
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Include verbose output or not')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'purr;help' {
            [CompletionResult]::new('generate', 'generate', [CompletionResultType]::ParameterValue, 'Generate a new config file')
            [CompletionResult]::new('config-path', 'config-path', [CompletionResultType]::ParameterValue, 'Return default config file path')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'purr;help;generate' {
            break
        }
        'purr;help;config-path' {
            break
        }
        'purr;help;help' {
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}

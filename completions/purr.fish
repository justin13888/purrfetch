# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_purr_global_optspecs
	string join \n verbose c/config= no-config all n/neofetch json ascii_distro= ascii_colors= no_ascii_bold L/logo off backend= source= separator= no_bold underline_char= title_fqdn colors= stdout memory_unit= uptime_shorthand= cpu_cores= h/help V/version
end

function __fish_purr_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_purr_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_purr_using_subcommand
	set -l cmd (__fish_purr_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c purr -n "__fish_purr_needs_command" -s c -l config -d 'Path to a custom config file' -r -F
complete -c purr -n "__fish_purr_needs_command" -l ascii_distro -d 'Force a specific distro logo (e.g. "arch")' -r
complete -c purr -n "__fish_purr_needs_command" -l ascii_colors -d 'Override logo colours (space/comma list, e.g. "4 6 1")' -r
complete -c purr -n "__fish_purr_needs_command" -l backend -d 'Logo backend: ascii or kitty' -r
complete -c purr -n "__fish_purr_needs_command" -l source -d 'Image source (PNG) for the kitty backend' -r -F
complete -c purr -n "__fish_purr_needs_command" -l separator -d 'Separator between labels and values' -r
complete -c purr -n "__fish_purr_needs_command" -l underline_char -d 'Character used for the title underline' -r
complete -c purr -n "__fish_purr_needs_command" -l colors -d 'Override text colours (space/comma list)' -r
complete -c purr -n "__fish_purr_needs_command" -l memory_unit -d 'Memory unit: kib, mib, or gib' -r
complete -c purr -n "__fish_purr_needs_command" -l uptime_shorthand -d 'Uptime format: on, tiny, or off' -r
complete -c purr -n "__fish_purr_needs_command" -l cpu_cores -d 'CPU cores: logical, physical, or off' -r
complete -c purr -n "__fish_purr_needs_command" -l verbose -d 'Include verbose output or not'
complete -c purr -n "__fish_purr_needs_command" -l no-config -d 'Ignore any config file and start from the built-in defaults'
complete -c purr -n "__fish_purr_needs_command" -l all -d 'Use the all-probes preset'
complete -c purr -n "__fish_purr_needs_command" -s n -l neofetch -d 'Use the neofetch text renderer'
complete -c purr -n "__fish_purr_needs_command" -l json -d 'Emit JSON instead of text'
complete -c purr -n "__fish_purr_needs_command" -l no_ascii_bold -d 'Don\'t bold the logo'
complete -c purr -n "__fish_purr_needs_command" -s L -l logo -d 'Show only the logo (no info)'
complete -c purr -n "__fish_purr_needs_command" -l off -d 'Hide the logo'
complete -c purr -n "__fish_purr_needs_command" -l no_bold -d 'Don\'t bold the title and labels'
complete -c purr -n "__fish_purr_needs_command" -l title_fqdn -d 'Show the fully-qualified hostname'
complete -c purr -n "__fish_purr_needs_command" -l stdout -d 'Pipe-friendly output: disable colour'
complete -c purr -n "__fish_purr_needs_command" -s h -l help -d 'Print help'
complete -c purr -n "__fish_purr_needs_command" -s V -l version -d 'Print version'
complete -c purr -n "__fish_purr_needs_command" -f -a "generate" -d 'Generate a new config file'
complete -c purr -n "__fish_purr_needs_command" -f -a "config-path" -d 'Return default config file path'
complete -c purr -n "__fish_purr_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c purr -n "__fish_purr_using_subcommand generate" -s n -l neofetch -d 'Generate neofetch preset'
complete -c purr -n "__fish_purr_using_subcommand generate" -l all -d 'Use all default presets'
complete -c purr -n "__fish_purr_using_subcommand generate" -l verbose -d 'Include verbose output or not'
complete -c purr -n "__fish_purr_using_subcommand generate" -s h -l help -d 'Print help'
complete -c purr -n "__fish_purr_using_subcommand config-path" -l verbose -d 'Include verbose output or not'
complete -c purr -n "__fish_purr_using_subcommand config-path" -s h -l help -d 'Print help'
complete -c purr -n "__fish_purr_using_subcommand help; and not __fish_seen_subcommand_from generate config-path help" -f -a "generate" -d 'Generate a new config file'
complete -c purr -n "__fish_purr_using_subcommand help; and not __fish_seen_subcommand_from generate config-path help" -f -a "config-path" -d 'Return default config file path'
complete -c purr -n "__fish_purr_using_subcommand help; and not __fish_seen_subcommand_from generate config-path help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'

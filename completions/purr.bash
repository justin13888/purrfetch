_purr() {
    local i cur prev opts cmd
    COMPREPLY=()
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi
    prev="$3"
    cmd=""
    opts=""

    for i in "${COMP_WORDS[@]:0:COMP_CWORD}"
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="purr"
                ;;
            purr,config-path)
                cmd="purr__subcmd__config__subcmd__path"
                ;;
            purr,generate)
                cmd="purr__subcmd__generate"
                ;;
            purr,help)
                cmd="purr__subcmd__help"
                ;;
            purr__subcmd__help,config-path)
                cmd="purr__subcmd__help__subcmd__config__subcmd__path"
                ;;
            purr__subcmd__help,generate)
                cmd="purr__subcmd__help__subcmd__generate"
                ;;
            purr__subcmd__help,help)
                cmd="purr__subcmd__help__subcmd__help"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        purr)
            opts="-c -n -L -h -V --verbose --config --no-config --all --neofetch --json --ascii_distro --ascii_colors --no_ascii_bold --logo --off --backend --source --separator --no_bold --underline_char --title_fqdn --colors --stdout --memory_unit --uptime_shorthand --cpu_cores --help --version generate config-path help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --config)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -c)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --ascii_distro)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --ascii_colors)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --backend)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --source)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --separator)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --underline_char)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --colors)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --memory_unit)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --uptime_shorthand)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --cpu_cores)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        purr__subcmd__config__subcmd__path)
            opts="-h --verbose --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        purr__subcmd__generate)
            opts="-n -h --neofetch --all --verbose --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        purr__subcmd__help)
            opts="generate config-path help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        purr__subcmd__help__subcmd__config__subcmd__path)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        purr__subcmd__help__subcmd__generate)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        purr__subcmd__help__subcmd__help)
            opts=""
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _purr -o nosort -o bashdefault -o default purr
else
    complete -F _purr -o bashdefault -o default purr
fi

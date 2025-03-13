#!/bin/bash
DRY_RUN='false'

WORKINGDIR="$( cd "$(dirname "$0")" ; pwd -P )"
cd $WORKINGDIR

# Order: dependencies first (so "spotipi" using everything before it goes last)
crates=( "protocol" "oauth" "core" "discovery" "audio" "metadata" "playback" "connect" "spotipi" )

function replace_in_file() {
  OS=`uname`
  shopt -s nocasematch
  case "$OS" in
    darwin)
      # for macOS
      sed -i '' -e "$1" "$2"
      ;;
    *)
      # for Linux and Windows
      sed -i'' -e "$1" "$2"
      ;;
  esac
}

function run {
  for CRATE in "${crates[@]}"
  do
    if [ "$CRATE" = "spotipi" ]
    then
      CRATE_DIR=''
    else
      CRATE_DIR=$CRATE
    fi
    crate_path="$WORKINGDIR/$CRATE_DIR/Cargo.toml"
    crate_path=${crate_path//\/\///}
    $(replace_in_file "s/^version.*/version = \"$1\"/g" "$crate_path")
    echo "Path is $crate_path"
    if [ "$CRATE" = "spotipi" ]
    then
      echo "Updating lockfile"
      if [ "$DRY_RUN" = 'true' ] ; then
        cargo update --dry-run
        # git add . && git commit --dry-run -a -m "Update Cargo.lock"
      else
        cargo update
        # git add . && git commit -a -m "Update Cargo.lock"
      fi
    fi
  done
}

#Set Script Name variable
SCRIPT=`basename ${BASH_SOURCE[0]}`

print_usage () {
  local l_MSG=$1
  if [ ! -z "${l_MSG}" ]; then
    echo "Usage Error: $l_MSG"
  fi
  echo "Usage: $SCRIPT <args> <version>"
  echo "  where <version> specifies the version number in semver format, eg. 1.0.1"
  echo "Recognized optional command line arguments"
  echo "--dry-run -- Test the script before making live changes"
  exit 1
}

### check number of command line arguments
NUMARGS=$#
if [ $NUMARGS -eq 0 ]; then
  print_usage 'No command line arguments specified'
fi

while test $# -gt 0; do
  case "$1" in
    -h|--help)
      print_usage
      exit 0
      ;;
      --dry-run)
        DRY_RUN='true'
        shift
        ;;
    *)
      break
      ;;
  esac
done

# First argument is new version number.
run $1
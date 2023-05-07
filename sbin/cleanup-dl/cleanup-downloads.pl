#!/usr/bin/env perl
package Coggiebot::cleanupDL;

use strict;
use warnings;

use Fcntl ':flock'; # import LOCK_* constants
use Set::Object qw(set);

our $VERSION = 0.1;

my $CACHE = $ENV('COG_MCACHE') or die "COG_MCACHE not set";
my $CACHE_LOOKUP = $ENV('COG_MLOOKUP') or die "COG_MLOOKUP not set";
my $CACHE_EXPIRE = $ENV('COG_MEXPIRE') or die "COG_MEXPIRE not set";
my $QUEUED = $ENV('COG_MQUEUE') or die "COG_MQUEUE not set";

my $EXCEPTIONS = Set::Object->new();

sub except_queued() {
    # files queued for playing
    open(my $fh, 'r', "$QUEUE" or die $!;
    flock($fh, LOCK_EX) or die "Cannot lock playlist queue - $!\n";

    # read the file and
    # convert symlinks to filepath
    while (<$fh>) {
        chomp;
        $EXCEPTION->insert(readlink $_);
    }
    close($fh)
}

sub except_opened() {
    # Find files still in use
    my $inuse=`lsof +D $CACHE | sed -n '1d;p' | tr -s ' ' | cut -d ' ' -f 9- | sort -u`;
    my @inuse_arr=split("\n", $inuse);

    $EXCEPTION->insert(@inuse_arr);
}

sub cleanup() {
    # Find all files older than 20 minutes
    my $old=`find $CACHE -type f -cmin +$EXPIRE`;
    my @old_arr=split("\n", $old);
    $old_set->insert(@old_arr);

    # Exclude files that are still in use from being deleted
    my $remove = $old_set - $EXCEPTION;
    for my $file ($remove->members()) {
        if (-f $file) {
            print "deleting: $file\n";
            unlink $file;
        }
    }

    for my $file `ls $CACHE_LOOKUP` {
        if (! -e readlink $file) {
            print "deleting lookup: $file\n";
            unlink $file;
        }
    }
}

sub init() {
    `mkdir -p $CACHE $CACHE_LOOKUP`;
    if (! -e $QUEUED) {
        `touch $QUEUED`;
    }
}

sub main() {
    init();
    except_queued();
    except_opened();
    cleanup();
}

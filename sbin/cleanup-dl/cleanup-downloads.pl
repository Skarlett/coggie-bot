#!/usr/bin/env perl
package Coggiebot::cleanupDL;

use strict;
use warnings;

use Set::Object qw(set);

our $VERSION = 0.1;
my $EXPIRE = $ENV('DEEMIX_CTIME_EXPIRE') or die "DEEMIX_CTIME_EXPIRE not set";

my $EXCEPTIONS = Set::Object->new();
my $REMOVE = Set::Object->new();

sub except_opened {
    my $dir = $_[0] or die "No directory specified";
    # Find files still in use
    my $inuse=`lsof +D $dir | sed -n '1d;p' | tr -s ' ' | cut -d ' ' -f 9- | sort -u`;
    my @inuse_arr=split("\n", $inuse);
    $EXCEPTION->insert(@inuse_arr);
}

sub cleanup {
    my $dir = $_[0] or die "No directory specified";
    
    my $rmraw=`find $CACHE -type f -cmin +$EXPIRE`;
    my @rmlist=split("\n", $rmraw);
    $REMOVE->insert(@rmlist);

    # Exclude files that are still in use from being deleted
    my $removals = $REMOVE - $EXCEPTION;
    for my $file ($removals->members()) {
        if (-f $file) {
            print "deleting: $file\n";
            unlink $file;
        }
    }
}

sub main() {
    my $dir = $ARGV[0] or die "No directory specified";

    except_opened($dir);
    cleanup($dir);
}

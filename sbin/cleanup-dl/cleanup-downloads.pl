#!/usr/bin/env perl
package Coggiebot::cleanupDL;

use strict;
use warnings;
use Set::Object qw(set);

our $VERSION = 0.1;

if (scalar (@ARGV) == 0) {
    die "usage: ./exec [directory]";
}

elsif (! -d $ARGV[0]) {
    die "DIRECTORY NOT FOUND: $ARGV[0]";
}

my $inuse_set = Set::Object->new();
my $old_set=Set::Object->new();

# Find files still in use
my $inuse=`lsof +D $ARGV[0] | sed -n '1d;p' | tr -s ' ' | cut -d ' ' -f 9- | sort -u`;

my @inuse_arr=split("\n", $inuse);
$inuse_set->insert(@inuse_arr);

# Find all files older than 20 minutes
my $old=`find $ARGV[0] -type f -cmin +20`;
my @old_arr=split("\n", $old);
$old_set->insert(@old_arr);

# Exclude files that are still in use from being deleted
my $remove=$old_set-$inuse_set;
for my $file ($remove->members()) {
    if (-f $file) {
        print "deleting: $file\n";
        unlink $file;
    }
}

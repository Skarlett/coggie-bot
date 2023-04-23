#!/usr/bin/env perl
package Coggiebot::cleanupDL;
use strict;
use warnings;
use Set::Object qw(set);

our $VERSION = 0.1;

my $inuse=`lsof +D $1 | tr -s ' ' | cut -d ' ' -f 9-`;
my @inuse_arr=split("\n", $inuse);
my $inuse_set = Set::Object->new();
$inuse_set->insert(@inuse_arr);

my $old=`find $1 -ctime +30min`;
my @old_arr=split("\n", $old);
my $old_set=Set::Object->new();
$old_set->insert(@old_arr);

my $remove=$old_set - $inuse_set;
print(@remove);



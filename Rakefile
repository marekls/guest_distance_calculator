# frozen_string_literal: true

require "bundler/gem_tasks"
require "rb_sys/extensiontask"

task build: :compile

GEMSPEC = Gem::Specification.load("guest_distance_calculator.gemspec")

RbSys::ExtensionTask.new("guest_distance_calculator", GEMSPEC) do |ext|
  ext.lib_dir = "lib/guest_distance_calculator"
end

task default: :compile

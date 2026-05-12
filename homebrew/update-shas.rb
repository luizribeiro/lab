#!/usr/bin/env ruby
require "digest"
require "net/http"
require "uri"

REPO = "luizribeiro/lab"
SYSTEMS = {
  aarch64_linux:  { arch_block: "on_arm",   os_block: "on_linux", system: "aarch64-linux" },
  aarch64_darwin: { arch_block: "on_arm",   os_block: "on_macos", system: "aarch64-darwin" },
  x86_64_linux:   { arch_block: "on_intel", os_block: "on_linux", system: "x86_64-linux" },
}

def fetch(url)
  uri = URI(url)
  res = Net::HTTP.get_response(uri)
  res = Net::HTTP.get_response(URI(res["location"])) if res.is_a?(Net::HTTPRedirection)
  raise "fetch failed #{url}: #{res.code}" unless res.is_a?(Net::HTTPSuccess)
  res.body
end

def artifact_url(tag, system)
  "https://github.com/#{REPO}/releases/download/#{tag}/rafaello-#{tag}-#{system}.tar.gz"
end

def rewrite_formula(text, replacements)
  stack = []
  text.lines.map do |line|
    if (m = line.match(/^\s*(on_arm|on_intel|on_linux|on_macos)\s+do\b/))
      stack.push(m[1])
      line
    elsif line.match?(/^\s*end\b/) && !stack.empty?
      stack.pop
      line
    else
      pair = replacements.find { |r| (stack & [r[:arch], r[:os]]).size == 2 }
      if pair && line =~ /^(\s*)url\s+"[^"]*"/
        "#{$1}url \"#{pair[:url]}\"\n"
      elsif pair && line =~ /^(\s*)sha256\s+"[^"]*"/
        "#{$1}sha256 \"#{pair[:sha]}\"\n"
      else
        line
      end
    end
  end.join
end

formula_path, tag = ARGV
abort "usage: update-shas.rb <formula.rb> <tag>" unless formula_path && tag

text = File.read(formula_path)
version = tag.sub(/\Av/, "")
text = text.sub(/^(\s*)version\s+"[^"]*"/) { "#{Regexp.last_match(1)}version \"#{version}\"" }

replacements = SYSTEMS.values.map do |info|
  url = artifact_url(tag, info[:system])
  sha = Digest::SHA256.hexdigest(fetch(url))
  { arch: info[:arch_block], os: info[:os_block], url: url, sha: sha }
end

File.write(formula_path, rewrite_formula(text, replacements))

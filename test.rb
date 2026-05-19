require 'json'
include Math

module Api
  class UserController < ApplicationController
    def index
      puts "hello"
    end
    
    def show
      puts "world"
    end
  end
end

class User < ApplicationRecord
  has_many :posts
  validates :email, presence: true
  before_save :normalize_email

  def normalize_email
    self.email = email.downcase
  end
end

Rails.application.routes.draw do
  namespace :api do
    resources :users
    get 'status', to: 'status#index'
  end
end
